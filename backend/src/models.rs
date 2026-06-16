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
    pub ancient_buildings: HashMap<String, BuildingMeta>,
    pub concert_halls: HashMap<String, BuildingMeta>,
    pub simulation_defaults: SimDefaults,
    pub alert_thresholds: AlertThresholds,
    pub noise_defaults: NoiseDefaults,
    pub valid_site_ids: Vec<String>,
    pub valid_sensor_ids: Vec<String>,
    pub valid_building_ids: Vec<String>,
    pub valid_hall_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseDefaults {
    pub visitor_noise_level_db: f64,
    pub crowd_density_per_sqm: f64,
    pub max_visitors: u32,
    pub speech_level_db: f64,
    pub noise_frequency_hz: f64,
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
    VirtualExperience {
        params: VirtualExperienceRequest,
        reply: tokio::sync::oneshot::Sender<VirtualExperienceResult>,
    },
    NoiseSimulation {
        params: NoiseSimulationRequest,
        reply: tokio::sync::oneshot::Sender<NoiseSimulationResult>,
    },
}

pub enum AnalyzerRequest {
    AnalyzeSti {
        params: StiAnalysisParams,
        reply: tokio::sync::oneshot::Sender<SpeechIntelligibility>,
    },
    CompareAcoustics {
        params: AcousticComparisonRequest,
        reply: tokio::sync::oneshot::Sender<AcousticComparisonResult>,
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
    pub dynasty: Option<String>,
    pub era: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseSource {
    pub position: Vec3,
    pub sound_level_db: f64,
    pub source_type: String,
    pub frequency_hz: f64,
    #[serde(default)]
    pub direction: Option<Vec3>,
    #[serde(default = "default_directivity")]
    pub directivity_index: f64,
    #[serde(default)]
    pub group_id: Option<String>,
}

fn default_directivity() -> f64 { 3.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinauralImpulseResponse {
    pub left_ear: Vec<f64>,
    pub right_ear: Vec<f64>,
    pub sample_rate: u32,
    pub listener_position: Vec3,
    pub source_position: Vec3,
    pub itd_seconds: f64,
    pub ild_db: f64,
    #[serde(default)]
    pub azimuth_rad: f64,
    #[serde(default = "default_playback_mode")]
    pub playback_mode: String,
    #[serde(default)]
    pub headphone_optimized: bool,
    #[serde(default)]
    pub hrtf_notes: String,
}

fn default_playback_mode() -> String { "headphone".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualExperienceRequest {
    pub site_id: String,
    pub source_position: Vec3,
    pub listener_position: Vec3,
    pub speech_text: Option<String>,
    pub frequency: f64,
    pub include_noise: bool,
    pub noise_sources: Option<Vec<NoiseSource>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualExperienceResult {
    pub site_id: String,
    pub source_position: Vec3,
    pub listener_position: Vec3,
    pub direct_path: SoundPath,
    pub reflection_paths: Vec<SoundPath>,
    pub total_paths: Vec<SoundPath>,
    pub impulse_response: Vec<f64>,
    pub binaural_ir: BinauralImpulseResponse,
    pub sti_with_noise: f64,
    pub sti_without_noise: f64,
    pub speech_intelligibility: SpeechIntelligibility,
    pub echo_count: u32,
    pub echo_delay_1: f64,
    pub echo_delay_2: f64,
    pub echo_delay_3: f64,
    pub reverberation_time_t60: f64,
    pub sound_preservation_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticComparisonRequest {
    pub site_ids: Vec<String>,
    pub source_position: Option<Vec3>,
    pub listener_position: Option<Vec3>,
    pub frequency: f64,
    pub background_noise_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteAcousticMetrics {
    pub site_id: String,
    pub site_name: String,
    pub category: String,
    pub dynasty: Option<String>,
    pub reverb_time_t60: f64,
    pub reverb_time_edt: f64,
    pub clarity_c50: f64,
    pub definition_d50: f64,
    pub sti_value: f64,
    pub rasti_value: f64,
    pub sound_pressure_level: f64,
    pub center_time: f64,
    pub bass_ratio: f64,
    pub brilliance: f64,
    pub intimacy: f64,
    pub warmth: f64,
    pub loudness: f64,
    pub echo_strength: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticComparisonResult {
    pub sites: Vec<SiteAcousticMetrics>,
    pub comparison_metrics: Vec<ComparisonMetric>,
    pub best_for_speech: String,
    pub best_for_music: String,
    pub best_for_echo: String,
    pub overall_ranking: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonMetric {
    pub metric_name: String,
    pub metric_unit: String,
    pub values: std::collections::HashMap<String, f64>,
    pub best_site: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseSimulationRequest {
    pub site_id: String,
    pub source_position: Vec3,
    pub listener_position: Vec3,
    pub noise_sources: Vec<NoiseSource>,
    pub speech_level_db: f64,
    pub frequency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseSimulationResult {
    pub site_id: String,
    pub total_noise_level_db: f64,
    pub speech_level_db: f64,
    pub snr_db: f64,
    pub sti_clean: f64,
    pub sti_noisy: f64,
    pub sti_degradation: f64,
    pub noise_contribution: std::collections::HashMap<String, f64>,
    pub recommended_max_visitors: u32,
    pub crowd_noise_map: Vec<Vec<f64>>,
    pub grid_resolution: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingMeta {
    pub building_id: String,
    pub name: String,
    pub category: String,
    pub dynasty: Option<String>,
    pub era_year: Option<i32>,
    pub location: String,
    pub architecture_style: String,
    pub description: String,
    pub acoustic_features: Vec<String>,
    pub historical_significance: String,
    pub dimensions: Vec3,
    pub volume_cubic_meters: f64,
    pub seating_capacity: Option<u32>,
    pub wall_material: String,
    pub ceiling_material: String,
    pub floor_material: String,
    pub wall_absorption: f64,
    pub ceiling_absorption: f64,
    pub floor_absorption: f64,
    pub typical_reverb_t60: f64,
    pub geometry_type: String,
    pub center_position: Vec3,
    #[serde(default)]
    pub literature_references: Vec<String>,
    #[serde(default = "default_data_quality")]
    pub data_quality: String,
    #[serde(default)]
    pub absorption_notes: String,
}

fn default_data_quality() -> String { "estimated".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncientDynastyConfig {
    pub tang_dynasty: BuildingMeta,
    pub song_dynasty: BuildingMeta,
    pub ming_dynasty: BuildingMeta,
    pub qing_dynasty: BuildingMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernConcertHallConfig {
    pub shoemaker: BuildingMeta,
    pub berlin_philharmonie: BuildingMeta,
    pub boston_symphony: BuildingMeta,
}
