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


ANCIENT_BUILDINGS = {
    "tang_temple": {
        "name": "唐代明堂",
        "dynasty": "唐",
        "volume": 180000.0,
        "base_t60": 2.8,
        "base_spl": 72.0,
        "base_sti": 0.55,
        "wall_absorption": 0.08,
        "description": "唐代礼制建筑，规模宏大",
    },
    "song_temple": {
        "name": "宋代大庆殿",
        "dynasty": "宋",
        "volume": 24000.0,
        "base_t60": 2.0,
        "base_spl": 75.0,
        "base_sti": 0.62,
        "wall_absorption": 0.10,
        "description": "宋代宫殿正殿",
    },
    "ming_temple": {
        "name": "明代奉天殿",
        "dynasty": "明",
        "volume": 56700.0,
        "base_t60": 3.2,
        "base_spl": 70.0,
        "base_sti": 0.50,
        "wall_absorption": 0.06,
        "description": "明代故宫三大殿之首",
    },
    "qing_temple": {
        "name": "清代太和殿",
        "dynasty": "清",
        "volume": 63700.0,
        "base_t60": 3.0,
        "base_spl": 71.0,
        "base_sti": 0.52,
        "wall_absorption": 0.07,
        "description": "清代故宫三大殿之首",
    },
}

MODERN_HALLS = {
    "shoemaker_hall": {
        "name": "鞋盒式音乐厅",
        "style": "Shoebox",
        "volume": 16200.0,
        "base_t60": 1.8,
        "base_spl": 80.0,
        "base_sti": 0.68,
        "wall_absorption": 0.15,
        "description": "典型现代鞋盒式音乐厅",
    },
    "vineyard_hall": {
        "name": "葡萄园式音乐厅",
        "style": "Vineyard",
        "volume": 15000.0,
        "base_t60": 2.0,
        "base_spl": 82.0,
        "base_sti": 0.65,
        "wall_absorption": 0.18,
        "description": "葡萄园梯田式音乐厅",
    },
    "boston_hall": {
        "name": "波士顿交响乐厅",
        "style": "Classical",
        "volume": 17100.0,
        "base_t60": 1.9,
        "base_spl": 81.0,
        "base_sti": 0.67,
        "wall_absorption": 0.08,
        "description": "世界著名声学标杆",
    },
}


class MultiBuildingSimulator:
    """多建筑声学对比模拟器 - 模拟各朝代宫殿与现代音乐厅的声学特性"""

    def __init__(self, api_url: str = "http://localhost:8080",
                 mqtt_host: str = "localhost", mqtt_port: int = 1883,
                 include_ancient: bool = True, include_modern: bool = True,
                 noise_mode: bool = False, visitor_count: int = 100):
        self.api_url = api_url.rstrip("/")
        self.mqtt_host = mqtt_host
        self.mqtt_port = mqtt_port
        self.include_ancient = include_ancient
        self.include_modern = include_modern
        self.noise_mode = noise_mode
        self.visitor_count = visitor_count

        import paho.mqtt.client as mqtt
        self.mqtt_client = mqtt.Client(client_id="multi-building-sim")
        try:
            self.mqtt_client.connect(mqtt_host, mqtt_port, 60)
            self.mqtt_client.loop_start()
        except Exception as e:
            print(f"[WARN] MQTT连接失败: {e}")

    def generate_building_measurement(self, building_id: str, building_info: dict,
                                       frequency: float = 1000.0,
                                       noise_level_db: float = 30.0) -> dict:
        """生成单个建筑的声学测量数据"""
        import math

        base_t60 = building_info["base_t60"]
        base_spl = building_info["base_spl"]
        base_sti = building_info["base_sti"]

        freq_factor = 1.0 / (frequency / 1000.0) ** 0.3
        t60 = base_t60 * freq_factor * random.uniform(0.9, 1.1)
        t30 = t60 * random.uniform(0.85, 0.95)
        edt = t60 * random.uniform(0.75, 0.85)

        spl = base_spl + random.gauss(0, 3)
        air_atten = 0.005 * 10.0
        spl -= air_atten

        if self.noise_mode:
            noise_factor = 1.0 - (noise_level_db - 30.0) / 80.0
            noise_factor = max(0.2, min(1.0, noise_factor))
            sti = base_sti * noise_factor + random.gauss(0, 0.03)
        else:
            sti = base_sti + random.gauss(0, 0.03)

        sti = max(0.0, min(1.0, sti))
        rasti = max(0.0, min(1.0, sti * random.uniform(1.02, 1.08)))

        d50 = max(0, min(100, 45 + (sti - 0.5) * 120 + random.gauss(0, 5)))
        c50 = max(-10, min(30, 2 + (sti - 0.5) * 45 + random.gauss(0, 2)))

        ir_samples = 2048
        ir = []
        peak_idx = 50
        for i in range(ir_samples):
            t = (i - peak_idx) / 44100.0
            if t < 0:
                ir_val = 0.0
            else:
                decay = math.exp(-t * 6.9078 / base_t60)
                noise = random.gauss(0, 0.02)
                ir_val = decay * (1.0 if i == peak_idx else 0.3 * random.random()) + noise
            ir.append(ir_val)

        return {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "measurement_id": str(uuid.uuid4()),
            "site_id": building_id,
            "site_name": building_info["name"],
            "sensor_id": f"{building_id.upper()}-S01",
            "pulse_response": [round(v, 6) for v in ir],
            "reverb_time_t60": round(t60, 3),
            "reverb_time_t30": round(t30, 3),
            "reverb_time_edt": round(edt, 3),
            "sound_pressure_level": round(spl, 2),
            "temperature": round(random.uniform(18, 26), 1),
            "humidity": round(random.uniform(40, 65), 1),
            "wind_speed": round(random.uniform(0, 3), 1),
            "frequency": frequency,
        }

    def run_comparison_simulation(self) -> None:
        """运行声学对比模拟：一次性生成所有建筑的测量数据"""
        print("\n" + "=" * 60)
        print("  多建筑声学对比模拟")
        print("=" * 60)

        all_buildings = {}
        if self.include_ancient:
            all_buildings.update(ANCIENT_BUILDINGS)
        if self.include_modern:
            all_buildings.update(MODERN_HALLS)

        if not all_buildings:
            print("没有选择任何建筑类型")
            return

        frequencies = [125, 250, 500, 1000, 2000, 4000, 8000]
        noise_level = 55.0 if self.noise_mode else 30.0

        print(f"\n模拟建筑数: {len(all_buildings)}")
        print(f"频率数: {len(frequencies)}")
        print(f"噪声模式: {'启用' if self.noise_mode else '禁用'}")
        if self.noise_mode:
            print(f"游客人数: {self.visitor_count} 人")

        print("\n--- 声学测量 ---")
        for bid, binf in all_buildings.items():
            print(f"\n[{binf['name']}]")
            for freq in frequencies:
                m = self.generate_building_measurement(bid, binf, freq, noise_level)
                try:
                    resp = requests.post(
                        f"{self.api_url}/api/measurements",
                        json=m,
                        timeout=5,
                    )
                    if resp.status_code == 200:
                        print(f"  {freq:>5} Hz | T60={m['reverb_time_t60']:.2f}s | "
                              f"SPL={m['sound_pressure_level']:.1f}dB")
                    else:
                        print(f"  {freq:>5} Hz | HTTP {resp.status_code}")
                except Exception as e:
                    print(f"  {freq:>5} Hz | ERR {e}")

        print("\n--- 语音清晰度分析 ---")
        for bid, binf in all_buildings.items():
            base_sti = binf["base_sti"]
            if self.noise_mode:
                noise_factor = max(0.2, 1.0 - (noise_level - 30.0) / 80.0)
                sti_val = base_sti * noise_factor
            else:
                sti_val = base_sti

            sti_val = max(0.0, min(1.0, sti_val + random.gauss(0, 0.02)))
            quality = ("优秀" if sti_val >= 0.85 else "良好" if sti_val >= 0.75
                       else "中等" if sti_val >= 0.60 else "较差" if sti_val >= 0.45 else "差")

            print(f"  {binf['name']:20s} | STI={sti_val:.3f} ({quality})")

        if self.noise_mode:
            print("\n--- 噪声影响分析 ---")
            self._analyze_noise_impact(all_buildings)

    def _analyze_noise_impact(self, buildings: dict) -> None:
        """分析游客噪声对各建筑的影响"""
        print(f"\n游客人数: {self.visitor_count} 人")
        print(f"人均噪声级: 60 dB")
        print("-" * 40)

        for bid, binf in buildings.items():
            base_sti = binf["base_sti"]
            volume = binf["volume"]

            noise_level = 40.0 + 10.0 * math.log10(max(1, self.visitor_count) * 1e-3 * 1000)
            noise_level = min(noise_level, 90.0)

            snr = 70.0 - noise_level
            sti_reduction = max(0.05, 0.5 - snr / 50.0) if snr < 25 else 0.02
            noisy_sti = max(0.0, base_sti - sti_reduction)

            print(f"\n  {binf['name']}:")
            print(f"    背景噪声: {noise_level:.1f} dB")
            print(f"    信噪比: {snr:.1f} dB")
            print(f"    安静STI: {base_sti:.3f}")
            print(f"    噪声STI: {noisy_sti:.3f}")
            print(f"    下降幅度: {(sti_reduction * 100):.1f}%")

    def run_continuous(self, interval_sec: int = 120) -> None:
        """连续运行多建筑对比模拟"""
        print(f"开始多建筑连续模拟，每 {interval_sec} 秒一次")
        print("按 Ctrl+C 停止")

        try:
            while True:
                self.run_comparison_simulation()
                time.sleep(interval_sec)
        except KeyboardInterrupt:
            print("\n模拟器已停止")

    def close(self):
        try:
            self.mqtt_client.loop_stop()
            self.mqtt_client.disconnect()
        except Exception:
            pass


def main_multi_building():
    """多建筑模拟入口函数"""
    import argparse
    parser = argparse.ArgumentParser(description="多建筑声学对比模拟器")
    parser.add_argument("--api-url", default="http://localhost:8080", help="后端API地址")
    parser.add_argument("--mqtt-host", default="localhost", help="MQTT broker地址")
    parser.add_argument("--mqtt-port", type=int, default=1883, help="MQTT broker端口")
    parser.add_argument("--ancient", action="store_true", default=True, help="包含古代建筑")
    parser.add_argument("--modern", action="store_true", default=True, help="包含现代音乐厅")
    parser.add_argument("--noise", action="store_true", help="启用游客噪声模拟")
    parser.add_argument("--visitors", type=int, default=100, help="游客人数(噪声模式)")
    parser.add_argument("--mode", choices=["once", "continuous"], default="once",
                        help="运行模式")
    parser.add_argument("--interval", type=int, default=120, help="连续模式间隔(秒)")

    args = parser.parse_args()

    sim = MultiBuildingSimulator(
        api_url=args.api_url,
        mqtt_host=args.mqtt_host,
        mqtt_port=args.mqtt_port,
        include_ancient=args.ancient,
        include_modern=args.modern,
        noise_mode=args.noise,
        visitor_count=args.visitors,
    )

    try:
        if args.mode == "once":
            sim.run_comparison_simulation()
        else:
            sim.run_continuous(args.interval)
    finally:
        sim.close()


if __name__ == "__main__":
    import sys
    if "--multi-building" in sys.argv:
        sys.argv.remove("--multi-building")
        main_multi_building()
    else:
        main()
