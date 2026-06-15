CREATE DATABASE IF NOT EXISTS tiantan_acoustics
    ENGINE = Atomic
    COMMENT '天坛声学仿真与语音清晰度分析数据库';

USE tiantan_acoustics;

CREATE TABLE IF NOT EXISTS acoustic_measurements
(
    timestamp       DateTime64(3) DEFAULT now64(3),
    measurement_id  UUID DEFAULT generateUUIDv4(),
    site_id         LowCardinality(String),
    site_name       LowCardinality(String),
    sensor_id       String,
    pulse_response  Array(Float64),
    reverb_time_t60 Float64,
    reverb_time_t30 Float64,
    reverb_time_edt Float64,
    sound_pressure_level Float64,
    temperature     Float64,
    humidity        Float64,
    wind_speed      Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192,
         min_bytes_for_wide_part = '10M';

CREATE TABLE IF NOT EXISTS sound_propagation_paths
(
    timestamp       DateTime64(3) DEFAULT now64(3),
    path_id         UUID DEFAULT generateUUIDv4(),
    site_id         LowCardinality(String),
    source_position Tuple(Float64, Float64, Float64),
    receiver_position Tuple(Float64, Float64, Float64),
    reflection_count UInt8,
    path_points     Array(Tuple(Float64, Float64, Float64)),
    travel_distance Float64,
    travel_time     Float64,
    attenuation_db  Float64,
    frequency       Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, timestamp)
SETTINGS index_granularity = 4096;

CREATE TABLE IF NOT EXISTS speech_intelligibility
(
    timestamp       DateTime64(3) DEFAULT now64(3),
    analysis_id     UUID DEFAULT generateUUIDv4(),
    site_id         LowCardinality(String),
    site_name       LowCardinality(String),
    sti_value       Float64,
    rasti_value     Float64,
    crispness       Float64,
    definition_d50  Float64,
    clarity_c50     Float64,
    center_time     Float64,
    frequency_bands Array(Float64),
    band_snr        Array(Float64),
    speech_content  Nullable(String)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, timestamp)
SETTINGS index_granularity = 4096;

CREATE TABLE IF NOT EXISTS acoustic_alerts
(
    timestamp       DateTime64(3) DEFAULT now64(3),
    alert_id        UUID DEFAULT generateUUIDv4(),
    site_id         LowCardinality(String),
    alert_type      LowCardinality(String),
    severity        LowCardinality(String),
    metric_name     String,
    current_value   Float64,
    threshold_value Float64,
    description     String,
    mqtt_topic      String,
    acknowledged    UInt8 DEFAULT 0
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, severity, timestamp)
SETTINGS index_granularity = 4096;

CREATE TABLE IF NOT EXISTS sound_field_snapshots
(
    timestamp       DateTime64(3) DEFAULT now64(3),
    snapshot_id     UUID DEFAULT generateUUIDv4(),
    site_id         LowCardinality(String),
    grid_resolution UInt16,
    grid_points_x   UInt16,
    grid_points_y   UInt16,
    pressure_field  Array(Array(Float64)),
    frequency       Float64,
    source_position Tuple(Float64, Float64, Float64)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, timestamp)
TTL timestamp + INTERVAL 30 DAY
SETTINGS index_granularity = 2048;

CREATE TABLE IF NOT EXISTS sensor_metadata
(
    sensor_id       String,
    site_id         LowCardinality(String),
    site_name       LowCardinality(String),
    position        Tuple(Float64, Float64, Float64),
    sensor_type     LowCardinality(String),
    installed_date  Date,
    calibration_date Date,
    status          LowCardinality(String) DEFAULT 'active'
)
ENGINE = ReplacingMergeTree(calibration_date)
ORDER BY (sensor_id, site_id)
SETTINGS index_granularity = 256;

CREATE TABLE IF NOT EXISTS site_info
(
    site_id         LowCardinality(String),
    site_name       LowCardinality(String),
    site_type       LowCardinality(String),
    description     String,
    center_position Tuple(Float64, Float64, Float64),
    dimensions      Tuple(Float64, Float64, Float64),
    wall_material   LowCardinality(String),
    wall_absorption Float64,
    created_at      DateTime DEFAULT now()
)
ENGINE = ReplacingMergeTree(created_at)
ORDER BY site_id
SETTINGS index_granularity = 256;

INSERT INTO site_info (site_id, site_name, site_type, description, center_position, dimensions, wall_material, wall_absorption) VALUES
('huiyinbi', '回音壁', 'circular_wall', '天坛皇穹宇圆形围墙，直径61.5米，高3.72米', (0.0, 0.0, 0.0), (61.5, 3.72, 61.5), 'blue_brick', 0.05),
('sanyinshi', '三音石', 'stone_plaza', '皇穹宇殿前甬道上的三块石板', (0.0, 0.0, 5.0), (10.0, 0.2, 2.0), 'limestone', 0.03),
('huanqiutan', '圜丘坛', 'circular_altar', '三层圆形石坛，上层直径23米，高5米', (0.0, 0.0, -30.0), (23.0, 5.0, 23.0), 'white_marble', 0.02);

INSERT INTO sensor_metadata (sensor_id, site_id, site_name, position, sensor_type, installed_date, calibration_date) VALUES
('HYB-S01', 'huiyinbi', '回音壁', (30.75, 1.8, 0.0), 'acoustic', '2024-01-15', '2026-01-10'),
('HYB-S02', 'huiyinbi', '回音壁', (-30.75, 1.8, 0.0), 'acoustic', '2024-01-15', '2026-01-10'),
('HYB-S03', 'huiyinbi', '回音壁', (0.0, 1.8, 30.75), 'acoustic', '2024-01-15', '2026-01-10'),
('HYB-S04', 'huiyinbi', '回音壁', (0.0, 1.8, -30.75), 'acoustic', '2024-01-15', '2026-01-10'),
('HYB-S05', 'huiyinbi', '回音壁', (0.0, 1.8, 0.0), 'acoustic', '2024-01-15', '2026-01-10'),
('SYS-S01', 'sanyinshi', '三音石', (0.0, 0.1, 4.0), 'acoustic', '2024-02-20', '2026-02-01'),
('SYS-S02', 'sanyinshi', '三音石', (0.0, 0.1, 5.0), 'acoustic', '2024-02-20', '2026-02-01'),
('SYS-S03', 'sanyinshi', '三音石', (0.0, 0.1, 6.0), 'acoustic', '2024-02-20', '2026-02-01'),
('HQT-S01', 'huanqiutan', '圜丘坛', (0.0, 5.0, -30.0), 'acoustic', '2024-03-10', '2026-03-01'),
('HQT-S02', 'huanqiutan', '圜丘坛', (11.5, 5.0, -30.0), 'acoustic', '2024-03-10', '2026-03-01'),
('HQT-S03', 'huanqiutan', '圜丘坛', (-11.5, 5.0, -30.0), 'acoustic', '2024-03-10', '2026-03-01');

CREATE MATERIALIZED VIEW IF NOT EXISTS alerts_summary_mv
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, alert_type, toStartOfDay(timestamp))
AS SELECT
    site_id,
    alert_type,
    severity,
    toStartOfDay(timestamp) AS day,
    count() AS total_alerts,
    countIf(acknowledged = 1) AS acknowledged_alerts
FROM acoustic_alerts
GROUP BY site_id, alert_type, severity, toStartOfDay(timestamp);

CREATE MATERIALIZED VIEW IF NOT EXISTS sti_hourly_avg_mv
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, toStartOfHour(timestamp))
AS SELECT
    site_id,
    toStartOfHour(timestamp) AS hour,
    count() AS sample_count,
    avg(sti_value) AS avg_sti,
    min(sti_value) AS min_sti,
    max(sti_value) AS max_sti,
    avg(clarity_c50) AS avg_c50
FROM speech_intelligibility
GROUP BY site_id, toStartOfHour(timestamp);
