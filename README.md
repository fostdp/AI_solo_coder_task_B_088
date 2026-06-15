# 天坛回音壁声学仿真与语音清晰度分析系统

## 系统架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Nginx (前端 + 反向代理)                        │
│                    ┌─────────────────────────┐                      │
│                    │  temple_of_heaven_3d.js  │                      │
│                    │   acoustic_panel.js      │                      │
│                    └─────────┬───────────────┘                      │
│                              │ /api/*                              │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────────┐
│                    Rust 后端 (Axum)                                   │
│                              │                                        │
│  ┌──────────────┐   mpsc   ┌┴──────────────┐   oneshot  ┌──────────┐│
│  │ dtu_receiver │───────→ │  alarm_mqtt   │←─────────│ acoustic││
│  │  数据采集校验  │          │ 告警评估推送   │           │_simulator││
│  └──────────────┘          └───────────────┘           │ 声学仿真  ││
│                                      ↑                  └──────────┘│
│  ┌──────────────┐   mpsc             │                               │
│  │ clarity_     │────────────────────┘                               │
│  │ analyzer     │   oneshot RPC                                      │
│  │ STI语音清晰度│                                                    │
│  └──────────────┘                                                    │
│         │ Prometheus (:9090/metrics)                                 │
└─────────┼────────────────────────────────────────────────────────────┘
          │
┌─────────┼──────────────────┐  ┌───────────────────┐
│ ClickHouse ◄───────────────│  │  Mosquitto MQTT   │
│  (时序数据存储)             │  │   (消息代理)        │
│  降采样+TTL保留策略         │  │                    │
└────────────────────────────┘  └───────────────────┘
          ▲
┌─────────┼──────────────────┐
│ 天坛声学模拟器 (Python)     │
│ 可配声源位置和频率           │
└────────────────────────────┘
```

## 技术栈

| 类别 | 技术 |
|------|------|
| **Backend** | Rust/Axum 异步HTTP框架、tokio mpsc channel 模块间通信、metrics/Prometheus 指标采集、tracing JSON 结构化日志 |
| **Storage** | ClickHouse 列式数据库、降采样物化视图 + TTL 自动过期保留策略 |
| **Messaging** | Mosquitto MQTT Broker、rumqttc Rust MQTT客户端 |
| **Frontend** | Three.js InstancedMesh 3D渲染、Nginx + Gzip 静态资源/反向代理 |
| **Simulation** | 几何声学(镜像源法) + 波动声学(声场网格) + UTD衍射、IEC 60268-16 STI（含古代汉语声调权重） |
| **Infrastructure** | Docker 多阶段构建、docker-compose 编排 |

## 快速部署

### 前置条件

- Docker 24+ 及 Docker Compose V2
- 4GB+ 可用内存（ClickHouse 约需 2GB）
- Git

### 一键启动

```bash
git clone <repo>
cd AI_solo_coder_task_A_088
docker compose up -d
```

### 分步部署

1. 启动 ClickHouse + MQTT：
   ```bash
   docker compose up -d clickhouse mosquitto
   ```
2. 等待 ClickHouse 就绪：
   ```bash
   docker compose logs -f clickhouse
   ```
   直到输出 "Ready for connections"
3. 启动后端：
   ```bash
   docker compose up -d backend
   ```
4. 启动前端：
   ```bash
   docker compose up -d frontend
   ```
5. 访问 http://localhost

### 验证部署

```bash
curl http://localhost/api/health
curl http://localhost:9090/metrics
```

## 模拟器用法

### Docker 运行

```bash
docker compose --profile simulator up simulator
```

### 直接运行

```bash
pip install requests paho-mqtt
python scripts/tiantan_simulator.py --api-url http://localhost:8080 --mqtt-host localhost
```

### 声源位置预设

| 预设名称 | 坐标 (x, y, z) | 说明 |
|----------|-----------------|------|
| `center` | (0, 1.5, 0) | 圆心默认位置 |
| `wall_north` | (0, 1.8, 30) | 回音壁北侧 |
| `wall_south` | (0, 1.8, -30) | 回音壁南侧 |
| `altar_center` | (0, 5, -30) | 圜丘坛中心 |
| `stone_1` | (0, 0.1, 4) | 三音石第一块 |
| `stone_2` | (0, 0.1, 5) | 三音石第二块 |
| `stone_3` | (0, 0.1, 6) | 三音石第三块 |

使用 `--source-preset center` 即可选用预设，覆盖 `--source-x/y/z` 参数。

### 频率参数

```bash
# 单频模式，默认 1000Hz
python scripts/tiantan_simulator.py --frequency 1000

# 多频段模式
python scripts/tiantan_simulator.py --frequencies "500,1000,2000,4000"

# JSON配置文件模式
python scripts/tiantan_simulator.py --config sim_config.json
```

### JSON配置文件格式

```json
{
  "source_position": [0.0, 1.5, 0.0],
  "frequencies": [500, 1000, 2000, 4000],
  "sites": ["huiyinbi"],
  "interval_seconds": 60
}
```

### 异常模式

异常模式可模拟声学特性退化场景（STI大幅下降、混响时间异常缩短、声压级骤降）：

```bash
# 命令行参数启动异常模式
python scripts/tiantan_simulator.py --anomaly --anomaly-site 回音壁
```

在连续模式交互式终端中输入：

```
anomaly 回音壁    # 触发回音壁声学异常退化
normal            # 恢复正常模式
```

## API 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 系统健康检查 |
| GET | `/api/sites` | 获取所有声学场所信息 |
| GET | `/api/sensors` | 获取所有传感器元数据 |
| GET | `/api/measurements` | 获取最近声学测量数据（支持 `?site_id=&limit=` 过滤） |
| POST | `/api/measurements` | 提交声学测量数据（经DTU校验后存储） |
| GET | `/api/measurements/{site_id}` | 获取指定场所的测量数据 |
| GET | `/api/sound-paths` | 获取声传播路径（支持 `?site_id=&limit=` 过滤） |
| POST | `/api/sound-paths` | 保存声传播路径 |
| GET | `/api/intelligibility` | 获取语音清晰度分析结果（支持 `?site_id=&limit=` 过滤） |
| POST | `/api/intelligibility` | 保存STI分析结果 |
| GET | `/api/alerts` | 获取告警记录（支持 `?site_id=&limit=` 过滤） |
| POST | `/api/alerts` | 保存告警记录 |
| GET | `/api/sound-field/{site_id}` | 获取指定场所最新声场快照 |
| POST | `/api/simulate/acoustics` | 运行声学仿真（镜像源射线追踪） |
| POST | `/api/simulate/sti` | 运行STI语音清晰度分析 |
| POST | `/api/simulate/wave-field` | 运行波动声场仿真 |
| GET | `/api/stats` | 获取存储统计信息 |
| GET | `:9090/metrics` | Prometheus 指标端点 |

## Prometheus 指标

| 指标名称 | 类型 | 说明 |
|----------|------|------|
| `tiantan_measurements_received_total` | Counter | 接收的测量数据总数 |
| `tiantan_measurements_invalid_total` | Counter | 校验失败的无效数据总数 |
| `tiantan_simulations_total` | Counter | 仿真执行总数（标签：type, site） |
| `tiantan_simulation_duration_seconds` | Histogram | 仿真执行耗时（标签：type） |
| `tiantan_simulation_failures_total` | Counter | 仿真失败总数 |
| `tiantan_latest_sti` | Gauge | 最新STI值（标签：site） |
| `tiantan_latest_spl_db` | Gauge | 最新声压级（标签：site, sensor） |
| `tiantan_latest_t60_seconds` | Gauge | 最新混响时间T60（标签：site, sensor） |
| `tiantan_http_request_duration_seconds` | Histogram | HTTP请求耗时（标签：endpoint） |

## ClickHouse 数据保留策略

| 表名 | 分辨率 | TTL | 说明 |
|------|--------|-----|------|
| `acoustic_measurements` | 原始数据 | 1 年 | 传感器原始测量数据 |
| `acoustic_measurements_5min` | 5分钟聚合 | 5 年 | SPL/T60 统计聚合（物化视图自动写入） |
| `sti_hourly_stats` | 小时聚合 | 5 年 | STI/D50/C50 趋势数据 |
| `sound_propagation_paths` | 原始数据 | 6 个月 | 射线追踪声路径结果 |
| `acoustic_alerts` | 原始数据 | 2 年 | 告警历史记录 |
| `sound_field_snapshots` | 原始数据 | 30 天 | 声场网格快照（大数据量） |
| `alerts_daily_summary` | 日聚合 | 3 年 | 告警统计汇总 |

## 配置文件

| 文件 | 说明 |
|------|------|
| `backend/config/acoustic_config.json` | 声学参数配置（场所几何参数、仿真默认值、告警阈值） |
| `backend/config/sti_weights.json` | STI权重配置（标准/古代汉语权重、声调增强参数） |
| `nginx/default.conf` | Nginx Gzip压缩 + 反向代理配置 |
| `mosquitto/mosquitto.conf` | MQTT Broker 配置 |

## 项目结构

```
├── backend/
│   ├── src/
│   │   ├── main.rs              # 入口+tracing+Prometheus初始化
│   │   ├── models.rs            # 数据模型+channel消息类型
│   │   ├── dtu_receiver.rs      # DTU数据采集校验模块
│   │   ├── acoustic_simulator.rs # 声学仿真引擎模块
│   │   ├── clarity_analyzer.rs  # STI语音清晰度模块
│   │   ├── alarm_mqtt.rs        # 告警评估MQTT推送模块
│   │   ├── routes.rs            # HTTP路由+指标采集
│   │   └── storage.rs           # ClickHouse存储层
│   ├── config/
│   │   ├── acoustic_config.json
│   │   └── sti_weights.json
│   ├── Cargo.toml
│   └── Dockerfile               # 多阶段构建
├── frontend/
│   ├── js/
│   │   ├── temple_of_heaven_3d.js  # Three.js 3D渲染模块
│   │   └── acoustic_panel.js       # 声学分析面板模块
│   └── index.html
├── scripts/
│   ├── tiantan_simulator.py     # 天坛声学模拟器
│   └── Dockerfile
├── sql/
│   └── init_clickhouse.sql      # 建表+降采样+TTL
├── nginx/
│   └── default.conf
├── mosquitto/
│   └── mosquitto.conf
├── docker-compose.yml
└── README.md
```
