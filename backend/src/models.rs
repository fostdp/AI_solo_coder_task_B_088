use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use nalgebra::Point3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self { Self { x, y, z } }
    pub fn to_point3(&self) -> Point3<f64> { Point3::new(self.x, self.y, self.z) }
    pub fn from_point3(p: &Point3<f64>) -> Self { Self { x: p.x, y: p.y, z: p.z } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticMeasurement {
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub measurement_id: Uuid,
    pub site_id: String,
    pub site_name: String,
    pub sensor_id: String,
    pub pulse_response: Vec<f64>,
    pub reverb_time_t60: f64,
    pub reverb_time_t30: f64,
    pub reverb_time_edt: f64,
    pub sound_pressure_level: f64,
    pub temperature: f64,
    pub humidity: f64,
    pub wind_speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundPath {
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub path_id: Uuid,
    pub site_id: String,
    pub source_position: Vec3,
    pub receiver_position: Vec3,
    pub reflection_count: u8,
    pub path_points: Vec<Vec3>,
    pub travel_distance: f64,
    pub travel_time: f64,
    pub attenuation_db: f64,
    pub frequency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechIntelligibility {
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub analysis_id: Uuid,
    pub site_id: String,
    pub site_name: String,
    pub sti_value: f64,
    pub rasti_value: f64,
    pub crispness: f64,
    pub definition_d50: f64,
    pub clarity_c50: f64,
    pub center_time: f64,
    pub frequency_bands: Vec<f64>,
    pub band_snr: Vec<f64>,
    pub speech_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticAlert {
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub alert_id: Uuid,
    pub site_id: String,
    pub alert_type: String,
    pub severity: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub description: String,
    pub mqtt_topic: String,
    pub acknowledged: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundFieldSnapshot {
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub snapshot_id: Uuid,
    pub site_id: String,
    pub grid_resolution: u16,
    pub grid_points_x: u16,
    pub grid_points_y: u16,
    pub pressure_field: Vec<Vec<f64>>,
    pub frequency: f64,
    pub source_position: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorMetadata {
    pub sensor_id: String,
    pub site_id: String,
    pub site_name: String,
    pub position: Vec3,
    pub sensor_type: String,
    pub installed_date: String,
    pub calibration_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteInfo {
    pub site_id: String,
    pub site_name: String,
    pub site_type: String,
    pub description: String,
    pub center_position: Vec3,
    pub dimensions: Vec3,
    pub wall_material: String,
    pub wall_absorption: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationParams {
    pub site_id: String,
    pub source_position: Vec3,
    pub frequency: f64,
    pub max_reflections: u8,
    pub num_rays: u32,
    pub temperature: f64,
    pub humidity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StiAnalysisParams {
    pub site_id: String,
    pub impulse_response: Vec<f64>,
    pub sample_rate: u32,
    pub background_noise_level: f64,
    pub speech_level: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), message: String::new() }
    }
    pub fn error(msg: &str) -> Self {
        Self { success: false, data: None, message: msg.to_string() }
    }
}
