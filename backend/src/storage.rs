use crate::models::{
    AcousticMeasurement, SoundPath, SpeechIntelligibility,
    AcousticAlert, SoundFieldSnapshot, SensorMetadata, SiteInfo,
};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, debug};
use parking_lot::Mutex;

pub type ChRow = Vec<(String, clickhouse::Value)>;

pub struct ClickHouseStore {
    url: String,
    database: String,
    fallback_buffer: Arc<Mutex<FallbackBuffer>>,
}

struct FallbackBuffer {
    measurements: Vec<AcousticMeasurement>,
    paths: Vec<SoundPath>,
    intelligibility: Vec<SpeechIntelligibility>,
    alerts: Vec<AcousticAlert>,
    fields: Vec<SoundFieldSnapshot>,
}

impl FallbackBuffer {
    fn new() -> Self {
        Self {
            measurements: Vec::new(),
            paths: Vec::new(),
            intelligibility: Vec::new(),
            alerts: Vec::new(),
            fields: Vec::new(),
        }
    }
}

impl ClickHouseStore {
    pub fn new(url: &str, database: &str) -> Self {
        Self {
            url: url.to_string(),
            database: database.to_string(),
            fallback_buffer: Arc::new(Mutex::new(FallbackBuffer::new())),
        }
    }

    pub async fn insert_measurement(&self, m: &AcousticMeasurement) -> anyhow::Result<()> {
        debug!("Inserting measurement for sensor {}", m.sensor_id);
        let buf = self.fallback_buffer.lock();
        self.fallback_buffer.lock().measurements.push(m.clone());
        Ok(())
    }

    pub async fn insert_sound_path(&self, p: &SoundPath) -> anyhow::Result<()> {
        self.fallback_buffer.lock().paths.push(p.clone());
        Ok(())
    }

    pub async fn batch_insert_paths(&self, paths: &[SoundPath]) -> anyhow::Result<()> {
        let mut buf = self.fallback_buffer.lock();
        buf.paths.extend_from_slice(paths);
        Ok(())
    }

    pub async fn insert_intelligibility(&self, s: &SpeechIntelligibility) -> anyhow::Result<()> {
        self.fallback_buffer.lock().intelligibility.push(s.clone());
        Ok(())
    }

    pub async fn insert_alert(&self, a: &AcousticAlert) -> anyhow::Result<()> {
        self.fallback_buffer.lock().alerts.push(a.clone());
        Ok(())
    }

    pub async fn insert_sound_field(&self, f: &SoundFieldSnapshot) -> anyhow::Result<()> {
        self.fallback_buffer.lock().fields.push(f.clone());
        Ok(())
    }

    pub async fn get_recent_measurements(
        &self,
        site_id: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<AcousticMeasurement>> {
        let buf = self.fallback_buffer.lock();
        let results: Vec<AcousticMeasurement> = buf.measurements
            .iter()
            .filter(|m| m.site_id == site_id)
            .rev()
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(results)
    }

    pub async fn get_recent_paths(
        &self,
        site_id: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<SoundPath>> {
        let buf = self.fallback_buffer.lock();
        let results: Vec<SoundPath> = buf.paths
            .iter()
            .filter(|p| p.site_id == site_id)
            .rev()
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(results)
    }

    pub async fn get_recent_intelligibility(
        &self,
        site_id: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<SpeechIntelligibility>> {
        let buf = self.fallback_buffer.lock();
        let results: Vec<SpeechIntelligibility> = buf.intelligibility
            .iter()
            .filter(|s| s.site_id == site_id)
            .rev()
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(results)
    }

    pub async fn get_recent_alerts(
        &self,
        site_id: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<AcousticAlert>> {
        let buf = self.fallback_buffer.lock();
        let results: Vec<AcousticAlert> = buf.alerts
            .iter()
            .filter(|a| site_id.map_or(true, |s| a.site_id == s))
            .rev()
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(results)
    }

    pub async fn get_latest_sound_field(
        &self,
        site_id: &str,
    ) -> anyhow::Result<Option<SoundFieldSnapshot>> {
        let buf = self.fallback_buffer.lock();
        Ok(buf.fields
            .iter()
            .filter(|f| f.site_id == site_id)
            .last()
            .cloned())
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<SensorMetadata>> {
        Ok(vec![
            SensorMetadata { sensor_id: "HYB-S01".into(), site_id: "huiyinbi".into(), site_name: "回音壁".into(),
                position: crate::models::Vec3::new(30.75, 1.8, 0.0), sensor_type: "acoustic".into(),
                installed_date: "2024-01-15".into(), calibration_date: "2026-01-10".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HYB-S02".into(), site_id: "huiyinbi".into(), site_name: "回音壁".into(),
                position: crate::models::Vec3::new(-30.75, 1.8, 0.0), sensor_type: "acoustic".into(),
                installed_date: "2024-01-15".into(), calibration_date: "2026-01-10".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HYB-S03".into(), site_id: "huiyinbi".into(), site_name: "回音壁".into(),
                position: crate::models::Vec3::new(0.0, 1.8, 30.75), sensor_type: "acoustic".into(),
                installed_date: "2024-01-15".into(), calibration_date: "2026-01-10".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HYB-S04".into(), site_id: "huiyinbi".into(), site_name: "回音壁".into(),
                position: crate::models::Vec3::new(0.0, 1.8, -30.75), sensor_type: "acoustic".into(),
                installed_date: "2024-01-15".into(), calibration_date: "2026-01-10".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HYB-S05".into(), site_id: "huiyinbi".into(), site_name: "回音壁".into(),
                position: crate::models::Vec3::new(0.0, 1.8, 0.0), sensor_type: "acoustic".into(),
                installed_date: "2024-01-15".into(), calibration_date: "2026-01-10".into(), status: "active".into() },
            SensorMetadata { sensor_id: "SYS-S01".into(), site_id: "sanyinshi".into(), site_name: "三音石".into(),
                position: crate::models::Vec3::new(0.0, 0.1, 4.0), sensor_type: "acoustic".into(),
                installed_date: "2024-02-20".into(), calibration_date: "2026-02-01".into(), status: "active".into() },
            SensorMetadata { sensor_id: "SYS-S02".into(), site_id: "sanyinshi".into(), site_name: "三音石".into(),
                position: crate::models::Vec3::new(0.0, 0.1, 5.0), sensor_type: "acoustic".into(),
                installed_date: "2024-02-20".into(), calibration_date: "2026-02-01".into(), status: "active".into() },
            SensorMetadata { sensor_id: "SYS-S03".into(), site_id: "sanyinshi".into(), site_name: "三音石".into(),
                position: crate::models::Vec3::new(0.0, 0.1, 6.0), sensor_type: "acoustic".into(),
                installed_date: "2024-02-20".into(), calibration_date: "2026-02-01".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HQT-S01".into(), site_id: "huanqiutan".into(), site_name: "圜丘坛".into(),
                position: crate::models::Vec3::new(0.0, 5.0, -30.0), sensor_type: "acoustic".into(),
                installed_date: "2024-03-10".into(), calibration_date: "2026-03-01".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HQT-S02".into(), site_id: "huanqiutan".into(), site_name: "圜丘坛".into(),
                position: crate::models::Vec3::new(11.5, 5.0, -30.0), sensor_type: "acoustic".into(),
                installed_date: "2024-03-10".into(), calibration_date: "2026-03-01".into(), status: "active".into() },
            SensorMetadata { sensor_id: "HQT-S03".into(), site_id: "huanqiutan".into(), site_name: "圜丘坛".into(),
                position: crate::models::Vec3::new(-11.5, 5.0, -30.0), sensor_type: "acoustic".into(),
                installed_date: "2024-03-10".into(), calibration_date: "2026-03-01".into(), status: "active".into() },
        ])
    }

    pub async fn get_sites(&self) -> anyhow::Result<Vec<SiteInfo>> {
        Ok(vec![
            SiteInfo {
                site_id: "huiyinbi".into(), site_name: "回音壁".into(), site_type: "circular_wall".into(),
                description: "天坛皇穹宇圆形围墙，直径61.5米，高3.72米".into(),
                center_position: crate::models::Vec3::new(0.0, 0.0, 0.0),
                dimensions: crate::models::Vec3::new(61.5, 3.72, 61.5),
                wall_material: "blue_brick".into(), wall_absorption: 0.05,
            },
            SiteInfo {
                site_id: "sanyinshi".into(), site_name: "三音石".into(), site_type: "stone_plaza".into(),
                description: "皇穹宇殿前甬道上的三块石板".into(),
                center_position: crate::models::Vec3::new(0.0, 0.0, 5.0),
                dimensions: crate::models::Vec3::new(10.0, 0.2, 2.0),
                wall_material: "limestone".into(), wall_absorption: 0.03,
            },
            SiteInfo {
                site_id: "huanqiutan".into(), site_name: "圜丘坛".into(), site_type: "circular_altar".into(),
                description: "三层圆形石坛，上层直径23米，高5米".into(),
                center_position: crate::models::Vec3::new(0.0, 0.0, -30.0),
                dimensions: crate::models::Vec3::new(23.0, 5.0, 23.0),
                wall_material: "white_marble".into(), wall_absorption: 0.02,
            },
        ])
    }

    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        let buf = self.fallback_buffer.lock();
        (
            buf.measurements.len(),
            buf.paths.len(),
            buf.intelligibility.len(),
            buf.alerts.len(),
            buf.fields.len(),
        )
    }
}
