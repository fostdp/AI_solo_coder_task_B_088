use crate::models::{
    AcousticComparisonRequest, AcousticComparisonResult, BuildingMeta,
    SiteAcousticMetrics, ComparisonMetric,
};
use std::collections::HashMap;
use std::sync::Arc;

pub struct EraComparator {
    config: Arc<crate::models::AcousticConfig>,
}

impl EraComparator {
    pub fn new(config: Arc<crate::models::AcousticConfig>) -> Self {
        Self { config }
    }

    pub fn compare_across_eras(
        &self,
        params: &AcousticComparisonRequest,
    ) -> AcousticComparisonResult {
        let mut site_metrics = Vec::new();
        for site_id in &params.site_ids {
            if let Some(building) = self.config.ancient_buildings.get(site_id) {
                let metrics = Self::building_to_metrics(building, params.frequency, params.background_noise_db);
                site_metrics.push(metrics);
            } else if let Some(hall) = self.config.concert_halls.get(site_id) {
                let metrics = Self::building_to_metrics(hall, params.frequency, params.background_noise_db);
                site_metrics.push(metrics);
            } else if let Some(site) = self.config.sites.get(site_id) {
                let metrics = Self::site_to_metrics(site_id, site, params.frequency, params.background_noise_db);
                site_metrics.push(metrics);
            }
        }
        Self::build_comparison_result(site_metrics)
    }

    pub fn get_era_comparison_summary(&self) -> EraComparisonSummary {
        let ancient_t60_avg: f64 = self.config.ancient_buildings
            .values()
            .map(|b| b.typical_reverb_t60)
            .sum::<f64>()
            / self.config.ancient_buildings.len().max(1) as f64;

        let modern_t60_avg: f64 = self.config.concert_halls
            .values()
            .map(|b| b.typical_reverb_t60)
            .sum::<f64>()
            / self.config.concert_halls.len().max(1) as f64;

        let ancient_volume_avg: f64 = self.config.ancient_buildings
            .values()
            .map(|b| b.volume_cubic_meters)
            .sum::<f64>()
            / self.config.ancient_buildings.len().max(1) as f64;

        let modern_volume_avg: f64 = self.config.concert_halls
            .values()
            .map(|b| b.volume_cubic_meters)
            .sum::<f64>()
            / self.config.concert_halls.len().max(1) as f64;

        let ancient_design_goal = if ancient_t60_avg > 2.5 {
            "Ritual atmosphere and echo effect".to_string()
        } else {
            "Speech intelligibility".to_string()
        };

        let modern_design_goal = if modern_t60_avg < 2.2 {
            "Music clarity and intimacy".to_string()
        } else {
            "Rich reverberation for orchestral music".to_string()
        };

        EraComparisonSummary {
            ancient_building_count: self.config.ancient_buildings.len(),
            modern_hall_count: self.config.concert_halls.len(),
            ancient_avg_t60: ancient_t60_avg,
            modern_avg_t60: modern_t60_avg,
            ancient_avg_volume: ancient_volume_avg,
            modern_avg_volume: modern_volume_avg,
            t60_difference: ancient_t60_avg - modern_t60_avg,
            ancient_design_goal,
            modern_design_goal,
            key_insights: vec![
                format!("Ancient Chinese architecture prioritizes long reverberation (T60 {:.1}s avg) for ritual atmosphere", ancient_t60_avg),
                format!("Modern concert halls target controlled T60 ({:.1}s avg) per ISO 3382-1 for music clarity", modern_t60_avg),
                format!("The ancient circular geometry creates unique echo effects not found in modern shoebox designs"),
                "Ancient designs use natural materials; modern designs use engineered absorption panels".to_string(),
            ],
        }
    }

    pub fn compare_t60_trends(&self) -> T60TrendComparison {
        let ancient_t60s: Vec<(String, f64)> = self.config.ancient_buildings
            .values()
            .map(|b| (b.building_id.clone(), b.typical_reverb_t60))
            .collect();

        let modern_t60s: Vec<(String, f64)> = self.config.concert_halls
            .values()
            .map(|b| (b.building_id.clone(), b.typical_reverb_t60))
            .collect();

        let ancient_std = Self::compute_std(&ancient_t60s.iter().map(|(_, v)| *v).collect::<Vec<_>>());
        let modern_std = Self::compute_std(&modern_t60s.iter().map(|(_, v)| *v).collect::<Vec<_>>());

        T60TrendComparison {
            ancient_t60s,
            modern_t60s,
            ancient_std_dev: ancient_std,
            modern_std_dev: modern_std,
            ancient_min_max: Self::min_max(&ancient_t60s.iter().map(|(_, v)| *v).collect::<Vec<_>>()),
            modern_min_max: Self::min_max(&modern_t60s.iter().map(|(_, v)| *v).collect::<Vec<_>>()),
        }
    }

    fn building_to_metrics(
        building: &BuildingMeta,
        frequency: f64,
        noise_db: f64,
    ) -> SiteAcousticMetrics {
        let volume = building.volume_cubic_meters;
        let surface_area = 2.0 * (building.dimensions.x * building.dimensions.z
            + building.dimensions.x * building.dimensions.y
            + building.dimensions.z * building.dimensions.y);
        let avg_absorption = (building.wall_absorption + building.ceiling_absorption + building.floor_absorption) / 3.0;

        let t60 = if avg_absorption > 0.001 {
            0.161 * volume / (surface_area * avg_absorption)
        } else {
            building.typical_reverb_t60
        };

        let t60 = t60.max(0.5).min(5.0);
        let freq_factor = (frequency / 1000.0).powf(0.3);
        let t60_at_freq = t60 / freq_factor;

        let c50 = 3.0 + (1.5 - t60_at_freq) * 5.0;
        let d50 = 50.0 + (1.5 - t60_at_freq) * 20.0;
        let sti = (0.4 + (1.5 - t60_at_freq) * 0.3).clamp(0.2, 0.95);

        let noise_factor = if noise_db > 30.0 {
            (1.0 - (noise_db - 30.0) / 80.0).max(0.2)
        } else {
            1.0
        };
        let sti_noisy = sti * noise_factor;

        let bass_ratio = 1.2 - avg_absorption * 2.0;
        let brilliance = 0.8 + avg_absorption;
        let intimacy = if volume < 10000.0 { 0.9 } else if volume < 30000.0 { 0.7 } else { 0.5 };
        let warmth = 0.5 + bass_ratio.clamp(0.5, 2.0) * 0.3;
        let loudness = (1.0 - t60_at_freq / 5.0).clamp(0.0, 1.0);
        let echo_strength = if building.geometry_type.contains("circular") { 0.9 } else { 0.4 };

        SiteAcousticMetrics {
            site_id: building.building_id.clone(),
            site_name: building.name.clone(),
            category: building.category.clone(),
            dynasty: building.dynasty.clone(),
            reverb_time_t60: t60_at_freq,
            reverb_time_edt: t60_at_freq * 0.8,
            clarity_c50: c50.clamp(-5.0, 20.0),
            definition_d50: d50.clamp(10.0, 90.0),
            sti_value: sti_noisy,
            rasti_value: sti_noisy * 1.05,
            sound_pressure_level: 70.0 + loudness * 20.0,
            center_time: t60_at_freq * 0.3,
            bass_ratio: bass_ratio.clamp(0.5, 2.0),
            brilliance: brilliance.clamp(0.0, 1.0),
            intimacy: intimacy.clamp(0.0, 1.0),
            warmth: warmth.clamp(0.0, 1.0),
            loudness,
            echo_strength: echo_strength.clamp(0.0, 1.0),
            description: building.description.clone(),
        }
    }

    fn site_to_metrics(
        site_id: &str,
        site: &crate::models::SiteConfig,
        frequency: f64,
        noise_db: f64,
    ) -> SiteAcousticMetrics {
        let (base_t60, base_spl, description, name, dynasty) = match site_id {
            "huiyinbi" => (2.2, 78.0, "天坛皇穹宇圆形围墙，直径61.5米，高3.72米", "回音壁", Some("明/清".to_string())),
            "sanyinshi" => (1.2, 72.0, "皇穹宇殿前甬道上的三块石板", "三音石", Some("明/清".to_string())),
            "huanqiutan" => (1.8, 75.0, "三层圆形石坛，上层直径23米，高5米", "圜丘坛", Some("明/清".to_string())),
            _ => (2.0, 75.0, "未知场所", site_id, None),
        };

        let freq_factor = (frequency / 1000.0).powf(0.3);
        let t60 = base_t60 / freq_factor;
        let c50 = 3.0 + (1.5 - t60) * 5.0;
        let d50 = 50.0 + (1.5 - t60) * 20.0;
        let sti = (0.4 + (1.5 - t60) * 0.3).clamp(0.2, 0.95);
        let noise_factor = if noise_db > 30.0 {
            (1.0 - (noise_db - 30.0) / 80.0).max(0.2)
        } else {
            1.0
        };
        let sti_noisy = sti * noise_factor;

        SiteAcousticMetrics {
            site_id: site_id.to_string(),
            site_name: name.to_string(),
            category: "ancient".to_string(),
            dynasty,
            reverb_time_t60: t60,
            reverb_time_edt: t60 * 0.8,
            clarity_c50: c50.clamp(-5.0, 20.0),
            definition_d50: d50.clamp(10.0, 90.0),
            sti_value: sti_noisy,
            rasti_value: sti_noisy * 1.05,
            sound_pressure_level: base_spl,
            center_time: t60 * 0.3,
            bass_ratio: 1.2,
            brilliance: 0.7,
            intimacy: if t60 < 1.5 { 0.8 } else { 0.5 },
            warmth: 0.6,
            loudness: (1.0 - t60 / 5.0).clamp(0.0, 1.0),
            echo_strength: if site_id == "huiyinbi" { 0.95 } else if site_id == "sanyinshi" { 0.85 } else { 0.6 },
            description: description.to_string(),
        }
    }

    fn build_comparison_result(site_metrics: Vec<SiteAcousticMetrics>) -> AcousticComparisonResult {
        let mut comparison_metrics = Vec::new();
        let metric_definitions = [
            ("reverb_time_t60", "s", "混响时间T60", |m: &SiteAcousticMetrics| m.reverb_time_t60, false),
            ("clarity_c50", "dB", "语言清晰度C50", |m: &SiteAcousticMetrics| m.clarity_c50, true),
            ("definition_d50", "%", "语言清晰度D50", |m: &SiteAcousticMetrics| m.definition_d50, true),
            ("sti_value", "", "语音传输指数STI", |m: &SiteAcousticMetrics| m.sti_value, true),
            ("rasti_value", "", "快速语音传输指数RASTI", |m: &SiteAcousticMetrics| m.rasti_value, true),
            ("sound_pressure_level", "dB", "声压级SPL", |m: &SiteAcousticMetrics| m.sound_pressure_level, false),
            ("center_time", "s", "中心时间Ts", |m: &SiteAcousticMetrics| m.center_time, false),
            ("bass_ratio", "", "低音比", |m: &SiteAcousticMetrics| m.bass_ratio, true),
            ("brilliance", "", "明亮度", |m: &SiteAcousticMetrics| m.brilliance, true),
            ("intimacy", "", "亲切感", |m: &SiteAcousticMetrics| m.intimacy, true),
            ("warmth", "", "温暖感", |m: &SiteAcousticMetrics| m.warmth, true),
            ("loudness", "", "响度", |m: &SiteAcousticMetrics| m.loudness, false),
            ("echo_strength", "", "回声强度", |m: &SiteAcousticMetrics| m.echo_strength, false),
        ];

        for (name, unit, desc, getter, higher_is_better) in metric_definitions.iter() {
            let mut values = HashMap::new();
            let mut best_value = if *higher_is_better { f64::NEG_INFINITY } else { f64::INFINITY };
            let mut best_site = String::new();

            for m in &site_metrics {
                let val = getter(m);
                values.insert(m.site_id.clone(), val);
                if *higher_is_better {
                    if val > best_value {
                        best_value = val;
                        best_site = m.site_id.clone();
                    }
                } else {
                    if val < best_value {
                        best_value = val;
                        best_site = m.site_id.clone();
                    }
                }
            }

            comparison_metrics.push(ComparisonMetric {
                metric_name: name.to_string(),
                metric_unit: unit.to_string(),
                values,
                best_site,
                description: desc.to_string(),
            });
        }

        let mut best_for_speech = String::new();
        let mut best_speech_score = f64::NEG_INFINITY;
        let mut best_for_music = String::new();
        let mut best_music_score = f64::NEG_INFINITY;
        let mut best_for_echo = String::new();
        let mut best_echo_score = f64::NEG_INFINITY;

        for m in &site_metrics {
            let speech_score = m.sti_value * 0.5 + m.clarity_c50 / 20.0 * 0.3 + m.definition_d50 / 100.0 * 0.2;
            let music_score = m.reverb_time_t60 / 3.0 * 0.4 + m.warmth * 0.3 + m.bass_ratio * 0.3;
            let echo_score = m.echo_strength * 0.6 + m.reverb_time_t60 / 4.0 * 0.4;

            if speech_score > best_speech_score {
                best_speech_score = speech_score;
                best_for_speech = m.site_id.clone();
            }
            if music_score > best_music_score {
                best_music_score = music_score;
                best_for_music = m.site_id.clone();
            }
            if echo_score > best_echo_score {
                best_echo_score = echo_score;
                best_for_echo = m.site_id.clone();
            }
        }

        let mut ranked: Vec<SiteAcousticMetrics> = site_metrics.clone();
        ranked.sort_by(|a, b| b.sti_value.partial_cmp(&a.sti_value).unwrap());
        let overall_ranking: Vec<String> = ranked.iter().map(|m| m.site_id.clone()).collect();

        AcousticComparisonResult {
            sites: site_metrics,
            comparison_metrics,
            best_for_speech,
            best_for_music,
            best_for_echo,
            overall_ranking,
        }
    }

    fn compute_std(values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / (values.len() - 1) as f64;
        variance.sqrt()
    }

    fn min_max(values: &[f64]) -> (f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0);
        }
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for &v in values {
            if v < min { min = v; }
            if v > max { max = v; }
        }
        (min, max)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EraComparisonSummary {
    pub ancient_building_count: usize,
    pub modern_hall_count: usize,
    pub ancient_avg_t60: f64,
    pub modern_avg_t60: f64,
    pub ancient_avg_volume: f64,
    pub modern_avg_volume: f64,
    pub t60_difference: f64,
    pub ancient_design_goal: String,
    pub modern_design_goal: String,
    pub key_insights: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T60TrendComparison {
    pub ancient_t60s: Vec<(String, f64)>,
    pub modern_t60s: Vec<(String, f64)>,
    pub ancient_std_dev: f64,
    pub modern_std_dev: f64,
    pub ancient_min_max: (f64, f64),
    pub modern_min_max: (f64, f64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BuildingMeta, Vec3, AcousticConfig};
    use std::collections::HashMap;

    fn create_test_config() -> Arc<AcousticConfig> {
        let mut config = AcousticConfig {
            sites: HashMap::new(),
            ancient_buildings: HashMap::new(),
            concert_halls: HashMap::new(),
            simulation_defaults: Default::default(),
            alert_thresholds: Default::default(),
            noise_defaults: Default::default(),
            valid_site_ids: vec![],
            valid_sensor_ids: vec![],
            valid_building_ids: vec![],
            valid_hall_ids: vec![],
        };

        let ancient = BuildingMeta {
            building_id: "ancient_temple".to_string(),
            name: "Ancient Temple".to_string(),
            category: "ancient".to_string(),
            dynasty: Some("qing".to_string()),
            dimensions: Vec3 { x: 60.0, y: 30.0, z: 50.0 },
            volume_cubic_meters: 90000.0,
            wall_absorption: 0.10,
            ceiling_absorption: 0.08,
            floor_absorption: 0.04,
            typical_reverb_t60: 3.0,
            geometry_type: "rectangular".to_string(),
            description: "Ancient Chinese temple".to_string(),
            literature_references: vec![],
            data_quality: "estimated".to_string(),
            absorption_notes: "".to_string(),
        };

        let modern = BuildingMeta {
            building_id: "modern_hall".to_string(),
            name: "Modern Hall".to_string(),
            category: "modern".to_string(),
            dynasty: None,
            dimensions: Vec3 { x: 40.0, y: 15.0, z: 30.0 },
            volume_cubic_meters: 18000.0,
            wall_absorption: 0.20,
            ceiling_absorption: 0.25,
            floor_absorption: 0.15,
            typical_reverb_t60: 1.9,
            geometry_type: "shoebox".to_string(),
            description: "Modern concert hall".to_string(),
            literature_references: vec![],
            data_quality: "measured".to_string(),
            absorption_notes: "".to_string(),
        };

        config.ancient_buildings.insert("ancient_temple".to_string(), ancient);
        config.concert_halls.insert("modern_hall".to_string(), modern);
        Arc::new(config)
    }

    #[test]
    fn test_era_comparator_new() {
        let config = create_test_config();
        let comparator = EraComparator::new(config);
        assert_eq!(comparator.config.ancient_buildings.len(), 1);
        assert_eq!(comparator.config.concert_halls.len(), 1);
    }

    #[test]
    fn test_get_era_comparison_summary() {
        let config = create_test_config();
        let comparator = EraComparator::new(config);
        let summary = comparator.get_era_comparison_summary();
        assert_eq!(summary.ancient_building_count, 1);
        assert_eq!(summary.modern_hall_count, 1);
        assert!(summary.ancient_avg_t60 > summary.modern_avg_t60);
        assert!(summary.t60_difference > 0.0);
        assert_eq!(summary.key_insights.len(), 4);
    }

    #[test]
    fn test_compare_t60_trends() {
        let config = create_test_config();
        let comparator = EraComparator::new(config);
        let trends = comparator.compare_t60_trends();
        assert_eq!(trends.ancient_t60s.len(), 1);
        assert_eq!(trends.modern_t60s.len(), 1);
        assert!(trends.ancient_min_max.0 > 0.0);
    }

    #[test]
    fn test_building_to_metrics_ancient_vs_modern() {
        let config = create_test_config();
        let ancient = config.ancient_buildings.get("ancient_temple").unwrap();
        let modern = config.concert_halls.get("modern_hall").unwrap();

        let ancient_metrics = EraComparator::building_to_metrics(ancient, 1000.0, 40.0);
        let modern_metrics = EraComparator::building_to_metrics(modern, 1000.0, 40.0);

        assert!(ancient_metrics.reverb_time_t60 > modern_metrics.reverb_time_t60);
        assert!(ancient_metrics.echo_strength > modern_metrics.echo_strength);
    }

    #[test]
    fn test_compute_std() {
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let std = EraComparator::compute_std(&values);
        assert!((std - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_min_max() {
        let values = vec![3.0, 1.0, 5.0, 2.0, 4.0];
        let (min, max) = EraComparator::min_max(&values);
        assert_eq!(min, 1.0);
        assert_eq!(max, 5.0);
    }
}
