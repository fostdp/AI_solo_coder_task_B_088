#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
天坛声学模拟器 - 模拟传感器每分钟上报声学数据
支持模拟回音壁、三音石、圜丘坛的脉冲响应、混响时间、声压级
"""

import json
import time
import random
import math
import uuid
import argparse
import requests
import paho.mqtt.client as mqtt
from datetime import datetime, timezone
from dataclasses import dataclass, asdict, field
from typing import List, Dict, Tuple, Optional

SPEED_OF_SOUND = 343.0

SOURCE_PRESETS = {
    "center": (0.0, 1.5, 0.0),
    "wall_north": (0.0, 1.8, 30.0),
    "wall_south": (0.0, 1.8, -30.0),
    "altar_center": (0.0, 5.0, -30.0),
    "stone_1": (0.0, 0.1, 4.0),
    "stone_2": (0.0, 0.1, 5.0),
    "stone_3": (0.0, 0.1, 6.0),
}

SITES = {
    "huiyinbi": {
        "name": "回音壁",
        "radius": 30.75,
        "wall_height": 3.72,
        "wall_absorption": 0.05,
        "base_reverb_t60": 2.2,
        "base_spl": 78.0,
        "sensors": [
            {"id": "HYB-S01", "pos": (30.75, 1.8, 0.0)},
            {"id": "HYB-S02", "pos": (-30.75, 1.8, 0.0)},
            {"id": "HYB-S03", "pos": (0.0, 1.8, 30.75)},
            {"id": "HYB-S04", "pos": (0.0, 1.8, -30.75)},
            {"id": "HYB-S05", "pos": (0.0, 1.8, 0.0)},
        ],
    },
    "sanyinshi": {
        "name": "三音石",
        "radius": 5.0,
        "wall_height": 0.2,
        "wall_absorption": 0.03,
        "base_reverb_t60": 1.2,
        "base_spl": 72.0,
        "sensors": [
            {"id": "SYS-S01", "pos": (0.0, 0.1, 4.0)},
            {"id": "SYS-S02", "pos": (0.0, 0.1, 5.0)},
            {"id": "SYS-S03", "pos": (0.0, 0.1, 6.0)},
        ],
    },
    "huanqiutan": {
        "name": "圜丘坛",
        "radius": 11.5,
        "wall_height": 5.0,
        "wall_absorption": 0.02,
        "base_reverb_t60": 1.8,
        "base_spl": 75.0,
        "sensors": [
            {"id": "HQT-S01", "pos": (0.0, 5.0, -30.0)},
            {"id": "HQT-S02", "pos": (11.5, 5.0, -30.0)},
            {"id": "HQT-S03", "pos": (-11.5, 5.0, -30.0)},
        ],
    },
}

ANCIENT_SPEECHES = [
    "皇天在上，后土在下，今朕谨以玉帛牺牲，敢昭告于昊天上帝",
    "惟我中华，源远流长，敬天祭祖，祈福安康",
    "风调雨顺，国泰民安，五谷丰登，六畜兴旺",
    "天子祭天于圜丘，为民祈福，愿天命永驻",
    "馨香祷祝，诚意格天，伏惟尚飨",
]


@dataclass
class Measurement:
    site_id: str
    site_name: str
    sensor_id: str
    pulse_response: List[float]
    reverb_time_t60: float
    reverb_time_t30: float
    reverb_time_edt: float
    sound_pressure_level: float
    temperature: float
    humidity: float
    wind_speed: float
    frequency: float = 1000.0
    source_position: List[float] = field(default_factory=lambda: [0.0, 1.5, 0.0])


def compute_air_absorption(frequency: float) -> float:
    return 0.003 * (frequency / 1000.0) ** 1.5


class TiantanAcousticSimulator:
    def __init__(self, api_url: str, mqtt_host: str = "localhost", mqtt_port: int = 1883,
                 source_position: Tuple = (0.0, 1.5, 0.0), frequency: float = 1000.0,
                 frequencies: Optional[List[float]] = None,
                 active_sites: Optional[List[str]] = None):
        self.api_url = api_url.rstrip("/")
        self.mqtt_client = mqtt.Client(client_id=f"tiantan-simulator-{uuid.uuid4().hex[:8]}")
        self.mqtt_host = mqtt_host
        self.mqtt_port = mqtt_port
        self.anomaly_mode = False
        self.anomaly_site = None
        self.source_position = source_position
        self.frequency = frequency
        self.frequencies = frequencies or [frequency]
        self.active_sites = active_sites or list(SITES.keys())

        try:
            self.mqtt_client.connect(mqtt_host, mqtt_port, keepalive=60)
            self.mqtt_client.loop_start()
            print(f"[MQTT] Connected to {mqtt_host}:{mqtt_port}")
        except Exception as e:
            print(f"[MQTT] Connection failed: {e}, continuing with HTTP only")

    def generate_pulse_response(self, site: Dict, sensor_pos: Tuple,
                                 source_position: Optional[Tuple] = None,
                                 frequency: float = 1000.0,
                                 sample_rate: int = 44100,
                                 duration: float = 2.5) -> List[float]:
        src = source_position if source_position is not None else self.source_position
        num_samples = int(sample_rate * duration)
        ir = [0.0] * num_samples

        direct_dist = math.sqrt(sum((s - src[i]) ** 2 for i, s in enumerate(sensor_pos)))
        direct_idx = int(direct_dist / SPEED_OF_SOUND * sample_rate)
        if direct_idx < num_samples:
            air_atten_db = compute_air_absorption(frequency) * direct_dist
            direct_amp = 10.0 ** (-air_atten_db / 20.0)
            ir[direct_idx] = direct_amp

        r = site["radius"]
        num_reflections = random.randint(3, 8)
        for refl in range(1, num_reflections + 1):
            angle = random.uniform(0, 2 * math.pi)
            img_src = (r * math.cos(angle), src[1], r * math.sin(angle))
            dist = math.sqrt(sum((s - img_src[i]) ** 2 for i, s in enumerate(sensor_pos)))
            dist += refl * random.uniform(0.5, 2.0)
            idx = int(dist / SPEED_OF_SOUND * sample_rate)
            if idx < num_samples:
                amplitude = math.exp(-refl * 0.3) * (1.0 - site["wall_absorption"]) ** refl
                amplitude *= random.uniform(0.7, 1.0)
                air_atten_db = compute_air_absorption(frequency) * dist
                amplitude *= 10.0 ** (-air_atten_db / 20.0)
                ir[idx] += amplitude

        for i in range(num_samples):
            t = i / sample_rate
            decay = math.exp(-2.5 * t)
            ir[i] += random.gauss(0, 0.005) * decay

        peak = max(abs(v) for v in ir)
        if peak > 0:
            ir = [v / peak for v in ir]

        if self.anomaly_mode and self.anomaly_site == site["name"]:
            ir = [v * random.uniform(0.2, 0.4) for v in ir]

        downsample = max(1, len(ir) // 512)
        return ir[::downsample][:512]

    def compute_reverb(self, ir: List[float], sample_rate: int = 44100) -> Tuple[float, float, float]:
        energy = [x ** 2 for x in ir]
        total = sum(energy)
        if total < 1e-10:
            return 2.0, 1.8, 1.5

        cumulative = 0.0
        t_edt = t_t30 = t_t60 = 0.0
        found_edt = found_t30 = found_t60 = False

        for i, e in enumerate(energy):
            cumulative += e
            ratio = (total - cumulative) / total if total > 0 else 0
            if ratio <= 0:
                break
            decay_db = 10 * math.log10(ratio)
            t = i / sample_rate

            if not found_edt and decay_db <= -10:
                t_edt = t * 6
                found_edt = True
            if not found_t30 and decay_db <= -30:
                t_t30 = t * 2
                found_t30 = True
            if not found_t60 and decay_db <= -60:
                t_t60 = t
                found_t60 = True

        if not found_edt:
            t_edt = random.uniform(1.3, 1.8)
        if not found_t30:
            t_t30 = t_t60 * 0.9 if t_t60 > 0 else random.uniform(1.5, 2.0)
        if not found_t60:
            t_t60 = t_t30 / 0.9 if t_t30 > 0 else random.uniform(1.8, 2.5)

        return max(t_t60, 0.5), max(t_t30, 0.4), max(t_edt, 0.3)

    def generate_measurement(self, site_id: str, sensor: Dict,
                              frequency: float = 1000.0,
                              source_position: Optional[Tuple] = None) -> Measurement:
        site = SITES[site_id]
        sr = 44100
        ir = self.generate_pulse_response(site, sensor["pos"],
                                           source_position=source_position,
                                           frequency=frequency)
        t60, t30, edt = self.compute_reverb(ir, sr)

        base_t60 = site["base_reverb_t60"]
        if self.anomaly_mode and self.anomaly_site == site["name"]:
            t60 = random.uniform(0.3, 0.6)
            t30 = t60 * 0.85
            edt = t60 * 0.6

        freq_factor = (frequency / 1000.0) ** 0.3
        t60 = t60 * random.uniform(0.85, 1.15) / freq_factor
        t30 = t30 * random.uniform(0.85, 1.15) / freq_factor
        edt = edt * random.uniform(0.85, 1.15) / freq_factor

        spl = site["base_spl"] + random.gauss(0, 4)
        air_atten = compute_air_absorption(frequency) * 10.0
        spl -= air_atten
        if self.anomaly_mode and self.anomaly_site == site["name"]:
            spl = random.uniform(25, 35)

        temp = random.uniform(15, 30)
        humidity = random.uniform(30, 80)
        wind = random.uniform(0, 5)

        src = source_position if source_position is not None else self.source_position

        return Measurement(
            site_id=site_id,
            site_name=site["name"],
            sensor_id=sensor["id"],
            pulse_response=ir,
            reverb_time_t60=t60,
            reverb_time_t30=t30,
            reverb_time_edt=edt,
            sound_pressure_level=spl,
            temperature=temp,
            humidity=humidity,
            wind_speed=wind,
            frequency=frequency,
            source_position=list(src),
        )

    def send_measurement(self, m: Measurement) -> bool:
        payload = asdict(m)
        payload["timestamp"] = datetime.now(timezone.utc).isoformat()
        payload["measurement_id"] = str(uuid.uuid4())

        try:
            resp = requests.post(
                f"{self.api_url}/api/measurements",
                json=payload,
                timeout=5,
            )
            if resp.status_code == 200:
                print(f"  [OK] {m.sensor_id} f={m.frequency:.0f}Hz SPL={m.sound_pressure_level:.1f}dB T60={m.reverb_time_t60:.2f}s")
            else:
                print(f"  [HTTP {resp.status_code}] {resp.text}")
                return False
        except Exception as e:
            print(f"  [ERR] HTTP request failed: {e}")
            return False

        try:
            topic = f"tiantan/acoustics/{m.site_id}/measurements"
            self.mqtt_client.publish(topic, json.dumps(payload), qos=1)
        except Exception:
            pass

        return True

    def simulate_sti_analysis(self, site_id: str, frequency: float = 1000.0) -> None:
        site = SITES[site_id]
        base_sti = {
            "huiyinbi": 0.72,
            "sanyinshi": 0.65,
            "huanqiutan": 0.68,
        }.get(site_id, 0.60)

        if self.anomaly_mode and self.anomaly_site == site["name"]:
            base_sti = random.uniform(0.25, 0.40)

        freq_penalty = 0.02 * math.log10(frequency / 1000.0) if frequency > 0 else 0
        base_sti += freq_penalty

        sti_value = max(0.0, min(1.0, base_sti + random.gauss(0, 0.05)))
        rasti = max(0.0, min(1.0, sti_value * random.uniform(1.02, 1.10)))
        d50 = max(0, min(100, 50 + (sti_value - 0.5) * 150 + random.gauss(0, 5)))
        c50 = max(-10, min(30, 3 + (sti_value - 0.5) * 50 + random.gauss(0, 2)))

        payload = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "analysis_id": str(uuid.uuid4()),
            "site_id": site_id,
            "site_name": site["name"],
            "sti_value": round(sti_value, 4),
            "rasti_value": round(rasti, 4),
            "crispness": round(random.uniform(2, 8), 2),
            "definition_d50": round(d50, 2),
            "clarity_c50": round(c50, 2),
            "center_time": round(random.uniform(0.03, 0.08), 4),
            "frequency_bands": [125, 250, 500, 1000, 2000, 4000, 8000],
            "band_snr": [round(random.uniform(5, 25), 2) for _ in range(7)],
            "speech_content": random.choice(ANCIENT_SPEECHES),
        }

        try:
            resp = requests.post(
                f"{self.api_url}/api/intelligibility",
                json=payload,
                timeout=5,
            )
            quality = "优秀" if sti_value >= 0.85 else "良好" if sti_value >= 0.75 else "中等" if sti_value >= 0.60 else "较差" if sti_value >= 0.45 else "差"
            if resp.status_code == 200:
                print(f"  [STI] {site['name']} STI={sti_value:.3f} ({quality})")
            else:
                print(f"  [STI ERR] {resp.status_code}")
        except Exception as e:
            print(f"  [STI ERR] {e}")

    def run_once(self) -> None:
        print(f"\n=== [{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] 模拟传感器数据上报 ===")
        if self.anomaly_mode:
            print(f"*** 异常模式已启用: {self.anomaly_site} 声学特性异常退化 ***")

        active_site_ids = [sid for sid in SITES if sid in self.active_sites]

        for site_id in active_site_ids:
            site = SITES[site_id]
            print(f"\n[{site['name']}] {len(site['sensors'])} 个传感器, 频率: {self.frequencies}")
            for freq in self.frequencies:
                for sensor in site["sensors"]:
                    m = self.generate_measurement(site_id, sensor,
                                                   frequency=freq,
                                                   source_position=self.source_position)
                    self.send_measurement(m)

        print("\n[语音清晰度分析 (STI)]:")
        for site_id in active_site_ids:
            for freq in self.frequencies:
                self.simulate_sti_analysis(site_id, frequency=freq)

    def run_continuous(self, interval_sec: int = 60) -> None:
        print(f"开始连续模拟模式，每 {interval_sec} 秒上报一次数据")
        print(f"按 Ctrl+C 停止，输入 'anomaly <site_name>' 触发异常退化模拟")
        print(f"输入 'normal' 恢复正常模式")

        import threading
        import sys

        def input_thread():
            while True:
                try:
                    cmd = sys.stdin.readline().strip()
                    if cmd.startswith("anomaly"):
                        parts = cmd.split()
                        site = parts[1] if len(parts) > 1 else "回音壁"
                        self.anomaly_mode = True
                        self.anomaly_site = site
                        print(f"[模拟] 已触发 {site} 声学异常退化")
                    elif cmd == "normal":
                        self.anomaly_mode = False
                        self.anomaly_site = None
                        print("[模拟] 已恢复正常模式")
                except Exception:
                    break

        t = threading.Thread(target=input_thread, daemon=True)
        t.start()

        try:
            while True:
                self.run_once()
                time.sleep(interval_sec)
        except KeyboardInterrupt:
            print("\n模拟器已停止")

    def close(self):
        try:
            self.mqtt_client.loop_stop()
            self.mqtt_client.disconnect()
        except Exception:
            pass


def load_config(path: str) -> Dict:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def main():
    parser = argparse.ArgumentParser(description="天坛声学模拟器")
    parser.add_argument("--api-url", default="http://localhost:8080", help="后端API地址")
    parser.add_argument("--mqtt-host", default="localhost", help="MQTT broker地址")
    parser.add_argument("--mqtt-port", type=int, default=1883, help="MQTT broker端口")
    parser.add_argument("--mode", choices=["once", "continuous"], default="continuous", help="运行模式")
    parser.add_argument("--interval", type=int, default=60, help="连续模式上报间隔(秒)")
    parser.add_argument("--anomaly", action="store_true", help="启用异常模式(声学退化)")
    parser.add_argument("--anomaly-site", default="回音壁", help="异常退化的场所名")
    parser.add_argument("--source-x", type=float, default=0.0, help="声源X坐标")
    parser.add_argument("--source-y", type=float, default=1.5, help="声源Y坐标")
    parser.add_argument("--source-z", type=float, default=0.0, help="声源Z坐标")
    parser.add_argument("--frequency", type=float, default=1000, help="声源中心频率(Hz)")
    parser.add_argument("--frequencies", type=str, default=None,
                        help="多频段模式，逗号分隔频率列表，如 500,1000,2000,4000")
    parser.add_argument("--source-preset", type=str, default=None, choices=list(SOURCE_PRESETS.keys()),
                        help="声源位置预设，覆盖--source-x/y/z")
    parser.add_argument("--config", type=str, default=None,
                        help="JSON配置文件路径，加载站点/声源/频率设置")

    args = parser.parse_args()

    source_position = (args.source_x, args.source_y, args.source_z)
    frequency = args.frequency
    frequencies = None
    active_sites = list(SITES.keys())
    interval = args.interval

    if args.source_preset:
        source_position = SOURCE_PRESETS[args.source_preset]

    if args.frequencies:
        frequencies = [float(f.strip()) for f in args.frequencies.split(",")]

    if args.config:
        config = load_config(args.config)
        if "source_position" in config:
            sp = config["source_position"]
            source_position = (sp[0], sp[1], sp[2])
        if "frequencies" in config:
            frequencies = config["frequencies"]
        if "sites" in config:
            active_sites = config["sites"]
        if "interval_seconds" in config:
            interval = config["interval_seconds"]

    if frequencies is None:
        frequencies = [frequency]

    print("=" * 60)
    print("  天坛声学模拟器 - Acoustic Sensor Simulator")
    print("=" * 60)
    print(f"API: {args.api_url}")
    print(f"MQTT: {args.mqtt_host}:{args.mqtt_port}")
    print(f"模式: {args.mode}")
    print(f"声源位置: {source_position}")
    print(f"频率: {frequencies}")
    print(f"活跃站点: {active_sites}")

    sim = TiantanAcousticSimulator(
        args.api_url, args.mqtt_host, args.mqtt_port,
        source_position=source_position,
        frequency=frequency,
        frequencies=frequencies,
        active_sites=active_sites,
    )
    if args.anomaly:
        sim.anomaly_mode = True
        sim.anomaly_site = args.anomaly_site

    try:
        if args.mode == "once":
            sim.run_once()
        else:
            sim.run_continuous(interval)
    finally:
        sim.close()


if __name__ == "__main__":
    main()
