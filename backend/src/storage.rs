use crate::models::{
    AcousticMeasurement, SoundPath, SpeechIntelligibility,
    AcousticAlert, SoundFieldSnapshot, SensorMetadata, SiteInfo,
    AcousticConfig,
};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct ClickHouseStore {
    url: String,
    database: String,
    fallback_buffer: Arc<Mutex<FallbackBuffer>>,
    config: Arc<AcousticConfig>,
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
    pub fn new(url: &str, database: &str, config: Arc<AcousticConfig>) -> Self {
        Self {
            url: url.to_string(),
            database: database.to_string(),
            fallback_buffer: Arc::new(Mutex::new(FallbackBuffer::new())),
            config,
        }
    }

    pub async fn insert_measurement(&self, m: &AcousticMeasurement) -> anyhow::Result<()> {
        self.fallback_buffer.lock().measurements.push(m.clone());
        Ok(())
    }

    pub async fn insert_sound_path(&self, p: &SoundPath) -> anyhow::Result<()> {
        self.fallback_buffer.lock().paths.push(p.clone());
        Ok(())
    }

    pub async fn batch_insert_paths(&self, paths: &[SoundPath]) -> anyhow::Result<()> {
        self.fallback_buffer.lock().paths.extend_from_slice(paths);
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

    pub async fn get_recent_measurements(&self, site_id: &str, limit: u32) -> anyhow::Result<Vec<AcousticMeasurement>> {
        let buf = self.fallback_buffer.lock();
        Ok(buf.measurements.iter().filter(|m| m.site_id == site_id).rev().take(limit as usize).cloned().collect())
    }

    pub async fn get_recent_paths(&self, site_id: &str, limit: u32) -> anyhow::Result<Vec<SoundPath>> {
        let buf = self.fallback_buffer.lock();
        Ok(buf.paths.iter().filter(|p| p.site_id == site_id).rev().take(limit as usize).cloned().collect())
    }

    pub async fn get_recent_intelligibility(&self, site_id: &str, limit: u32) -> anyhow::Result<Vec<SpeechIntelligibility>> {
        let buf = self.fallback_buffer.lock();
        Ok(buf.intelligibility.iter().filter(|s| s.site_id == site_id).rev().take(limit as usize).cloned().collect())
    }

    pub async fn get_recent_alerts(&self, site_id: Option<&str>, limit: u32) -> anyhow::Result<Vec<AcousticAlert>> {
        let buf = self.fallback_buffer.lock();
        Ok(buf.alerts.iter().filter(|a| site_id.map_or(true, |s| a.site_id == s)).rev().take(limit as usize).cloned().collect())
    }

    pub async fn get_latest_sound_field(&self, site_id: &str) -> anyhow::Result<Option<SoundFieldSnapshot>> {
        let buf = self.fallback_buffer.lock();
        Ok(buf.fields.iter().filter(|f| f.site_id == site_id).last().cloned())
    }

    pub async fn get_sensors(&self) -> anyhow::Result<Vec<SensorMetadata>> {
        let sensors = vec![
            ("HYB-S01", "huiyinbi", "回音壁", 30.75, 1.8, 0.0),
            ("HYB-S02", "huiyinbi", "回音壁", -30.75, 1.8, 0.0),
            ("HYB-S03", "huiyinbi", "回音壁", 0.0, 1.8, 30.75),
            ("HYB-S04", "huiyinbi", "回音壁", 0.0, 1.8, -30.75),
            ("HYB-S05", "huiyinbi", "回音壁", 0.0, 1.8, 0.0),
            ("SYS-S01", "sanyinshi", "三音石", 0.0, 0.1, 4.0),
            ("SYS-S02", "sanyinshi", "三音石", 0.0, 0.1, 5.0),
            ("SYS-S03", "sanyinshi", "三音石", 0.0, 0.1, 6.0),
            ("HQT-S01", "huanqiutan", "圜丘坛", 0.0, 5.0, -30.0),
            ("HQT-S02", "huanqiutan", "圜丘坛", 11.5, 5.0, -30.0),
            ("HQT-S03", "huanqiutan", "圜丘坛", -11.5, 5.0, -30.0),
        ];
        Ok(sensors.into_iter().map(|(id, site, name, x, y, z)| SensorMetadata {
            sensor_id: id.into(), site_id: site.into(), site_name: name.into(),
            position: crate::models::Vec3::new(x, y, z), sensor_type: "acoustic".into(),
            installed_date: "2024-01-15".into(), calibration_date: "2026-01-10".into(), status: "active".into(),
        }).collect())
    }

    pub async fn get_sites(&self) -> anyhow::Result<Vec<SiteInfo>> {
        Ok(self.config.sites.iter().map(|(id, sc)| SiteInfo {
            site_id: id.clone(),
            site_name: sc.name.clone(),
            site_type: sc.site_type.clone(),
            description: sc.description.clone(),
            center_position: crate::models::Vec3::new(sc.center[0], sc.center[1], sc.center[2]),
            dimensions: crate::models::Vec3::new(sc.radius * 2.0, sc.wall_height, sc.radius * 2.0),
            wall_material: match sc.site_type.as_str() {
                "circular_wall" => "blue_brick",
                "stone_plaza" => "limestone",
                _ => "white_marble",
            }.to_string(),
            wall_absorption: sc.wall_absorption,
        }).collect())
    }

    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        let buf = self.fallback_buffer.lock();
        (buf.measurements.len(), buf.paths.len(), buf.intelligibility.len(), buf.alerts.len(), buf.fields.len())
    }
}
