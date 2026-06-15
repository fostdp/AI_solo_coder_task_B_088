use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use nalgebra::Point3;
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticConfig {
    pub sites: HashMap<String, SiteConfig>,
    pub simulation_defaults: SimDefaults,
    pub alert_thresholds: AlertThresholds,
    pub valid_site_ids: Vec<String>,
    pub valid_sensor_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub site_type: String,
    pub radius: f64,
    pub wall_height: f64,
    pub wall_absorption: f64,
    pub center: [f64; 3],
    pub base_reverb_t60: f64,
    pub base_spl: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimDefaults {
    pub speed_of_sound: f64,
    pub air_absorption_coefficient: f64,
    pub diffraction_threshold: f64,
    pub wave_field_grid_resolution: u16,
    pub wave_field_image_sources: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub sti_min: f64,
    pub rasti_min: f64,
    pub reverb_t60_min: f64,
    pub reverb_t60_max: f64,
    pub spl_min_db: f64,
    pub spl_max_db: f64,
    pub definition_d50_min: f64,
    pub clarity_c50_min: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StiWeightsConfig {
    pub octave_bands: Vec<f64>,
    pub modulation_frequencies: Vec<f64>,
    pub standard_weights: StiWeightSet,
    pub ancient_chinese_weights: AncientChineseWeightSet,
    pub snr_clamp_range: [f64; 2],
    pub default_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StiWeightSet {
    pub sti: Vec<f64>,
    pub rasti: Vec<f64>,
    pub rasti_bands: Vec<f64>,
    pub rasti_mod_freqs: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncientChineseWeightSet {
    pub sti: Vec<f64>,
    pub rasti: Vec<f64>,
    pub rasti_bands: Vec<f64>,
    pub rasti_mod_freqs: Vec<f64>,
    pub tone_critical_mod_freqs: Vec<f64>,
    pub tone_boost_factor: f64,
    pub tone_affected_bands: Vec<usize>,
}

pub enum DtuEvent {
    Measurement(AcousticMeasurement),
}

pub enum SimulatorRequest {
    TraceRays {
        params: SimulationParams,
        reply: tokio::sync::oneshot::Sender<Vec<SoundPath>>,
    },
    WaveField {
        params: SimulationParams,
        reply: tokio::sync::oneshot::Sender<SoundFieldSnapshot>,
    },
}

pub enum AnalyzerRequest {
    AnalyzeSti {
        params: StiAnalysisParams,
        reply: tokio::sync::oneshot::Sender<SpeechIntelligibility>,
    },
}

pub enum AlarmEvent {
    CheckMeasurement {
        site_id: String,
        reverb_t60: f64,
        spl: f64,
    },
    CheckIntelligibility {
        site_id: String,
        sti: f64,
        d50: f64,
        c50: f64,
    },
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
