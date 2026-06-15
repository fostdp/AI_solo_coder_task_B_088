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

CREATE TABLE IF NOT EXISTS acoustic_measurements_5min
(
    timestamp   DateTime64(3),
    site_id     LowCardinality(String),
    sensor_id   String,
    avg_spl     Float64,
    max_spl     Float64,
    min_spl     Float64,
    avg_t60     Float64,
    max_t60     Float64,
    min_t60     Float64,
    sample_count UInt32
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 5 YEAR;

CREATE MATERIALIZED VIEW IF NOT EXISTS acoustic_measurements_5min_mv
TO acoustic_measurements_5min
AS SELECT
    toStartOfFiveMinutes(timestamp) AS timestamp,
    site_id,
    sensor_id,
    avg(sound_pressure_level) AS avg_spl,
    max(sound_pressure_level) AS max_spl,
    min(sound_pressure_level) AS min_spl,
    avg(reverb_time_t60) AS avg_t60,
    max(reverb_time_t60) AS max_t60,
    min(reverb_time_t60) AS min_t60,
    count() AS sample_count
FROM acoustic_measurements
GROUP BY timestamp, site_id, sensor_id;

CREATE TABLE IF NOT EXISTS sti_hourly_stats
(
    timestamp   DateTime64(3),
    site_id     LowCardinality(String),
    avg_sti     Float64,
    min_sti     Float64,
    max_sti     Float64,
    avg_d50     Float64,
    avg_c50     Float64,
    sample_count UInt32
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, timestamp)
TTL timestamp + INTERVAL 5 YEAR;

DROP VIEW IF EXISTS sti_hourly_avg_mv;

CREATE MATERIALIZED VIEW IF NOT EXISTS sti_hourly_stats_mv
TO sti_hourly_stats
AS SELECT
    toStartOfHour(timestamp) AS timestamp,
    site_id,
    avg(sti_value) AS avg_sti,
    min(sti_value) AS min_sti,
    max(sti_value) AS max_sti,
    avg(definition_d50) AS avg_d50,
    avg(clarity_c50) AS avg_c50,
    count() AS sample_count
FROM speech_intelligibility
GROUP BY timestamp, site_id;

CREATE TABLE IF NOT EXISTS alerts_daily_summary
(
    day                 Date,
    site_id             LowCardinality(String),
    alert_type          LowCardinality(String),
    severity            LowCardinality(String),
    total_alerts        UInt32,
    acknowledged_alerts UInt32
)
ENGINE = SummingMergeTree()
ORDER BY (site_id, alert_type, day)
TTL day + INTERVAL 3 YEAR;

CREATE MATERIALIZED VIEW IF NOT EXISTS alerts_daily_summary_mv
TO alerts_daily_summary
AS SELECT
    toStartOfDay(timestamp) AS day,
    site_id,
    alert_type,
    severity,
    count() AS total_alerts,
    countIf(acknowledged = 1) AS acknowledged_alerts
FROM acoustic_alerts
GROUP BY day, site_id, alert_type, severity;

ALTER TABLE acoustic_measurements MODIFY TTL timestamp + INTERVAL 1 YEAR;
ALTER TABLE sound_propagation_paths MODIFY TTL timestamp + INTERVAL 6 MONTH;
ALTER TABLE acoustic_alerts MODIFY TTL timestamp + INTERVAL 2 YEAR;

-- ============================================================
-- 建筑元数据表 (新增)
-- ============================================================
CREATE TABLE IF NOT EXISTS building_meta
(
    building_id String,
    name String,
    category LowCardinality(String),
    dynasty Nullable(String),
    era_year Nullable(Int32),
    location String,
    architecture_style String,
    description String,
    acoustic_features Array(String),
    historical_significance String,
    dimensions_x Float64,
    dimensions_y Float64,
    dimensions_z Float64,
    volume_cubic_meters Float64,
    seating_capacity Nullable(UInt32),
    wall_material String,
    ceiling_material String,
    floor_material String,
    wall_absorption Float64,
    ceiling_absorption Float64,
    floor_absorption Float64,
    typical_reverb_t60 Float64,
    geometry_type LowCardinality(String),
    center_x Float64,
    center_y Float64,
    center_z Float64,
    created_at DateTime DEFAULT now()
)
ENGINE = MergeTree()
ORDER BY (building_id);

-- ============================================================
-- 声学对比分析结果表 (新增)
-- ============================================================
CREATE TABLE IF NOT EXISTS acoustic_comparison_results
(
    comparison_id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime DEFAULT now(),
    site_ids Array(String),
    frequency Float64,
    background_noise_db Float64,
    best_for_speech String,
    best_for_music String,
    best_for_echo String,
    overall_ranking Array(String),
    metric_names Array(String),
    metric_units Array(String),
    metric_descriptions Array(String)
)
ENGINE = MergeTree()
ORDER BY (timestamp, comparison_id)
TTL timestamp + INTERVAL 1 YEAR;

-- ============================================================
-- 噪声模拟结果表 (新增)
-- ============================================================
CREATE TABLE IF NOT EXISTS noise_simulation_results
(
    simulation_id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime DEFAULT now(),
    site_id LowCardinality(String),
    total_noise_level_db Float64,
    speech_level_db Float64,
    snr_db Float64,
    sti_clean Float64,
    sti_noisy Float64,
    sti_degradation Float64,
    recommended_max_visitors UInt32,
    grid_resolution UInt16,
    crowd_noise_map Array(Array(Float64))
)
ENGINE = MergeTree()
ORDER BY (site_id, timestamp)
TTL timestamp + INTERVAL 6 MONTH;

-- ============================================================
-- 虚拟体验记录表 (新增)
-- ============================================================
CREATE TABLE IF NOT EXISTS virtual_experience_logs
(
    experience_id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime DEFAULT now(),
    site_id LowCardinality(String),
    source_x Float64,
    source_y Float64,
    source_z Float64,
    listener_x Float64,
    listener_y Float64,
    listener_z Float64,
    speech_text Nullable(String),
    frequency Float64,
    include_noise UInt8,
    sti_without_noise Float64,
    sti_with_noise Float64,
    echo_count UInt32,
    echo_delay_1 Float64,
    echo_delay_2 Float64,
    echo_delay_3 Float64,
    reverberation_time_t60 Float64,
    sound_preservation_score Float64,
    itd_seconds Float64,
    ild_db Float64
)
ENGINE = MergeTree()
ORDER BY (site_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR;

-- ============================================================
-- 虚拟体验小时统计物化视图 (新增)
-- ============================================================
CREATE TABLE IF NOT EXISTS virtual_experience_hourly_stats
(
    site_id LowCardinality(String),
    hour DateTime,
    experience_count UInt64,
    avg_sti_clean Float64,
    avg_sti_noisy Float64,
    avg_echo_count Float64,
    avg_t60 Float64,
    avg_preservation_score Float64
)
ENGINE = SummingMergeTree()
ORDER BY (site_id, hour)
TTL hour + INTERVAL 2 YEAR;

CREATE MATERIALIZED VIEW IF NOT EXISTS virtual_experience_hourly_mv
TO virtual_experience_hourly_stats
AS SELECT
    site_id,
    toStartOfHour(timestamp) AS hour,
    count() AS experience_count,
    avg(sti_without_noise) AS avg_sti_clean,
    avg(sti_with_noise) AS avg_sti_noisy,
    avg(echo_count) AS avg_echo_count,
    avg(reverberation_time_t60) AS avg_t60,
    avg(sound_preservation_score) AS avg_preservation_score
FROM virtual_experience_logs
GROUP BY site_id, hour;
