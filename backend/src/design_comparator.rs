use crate::models::{
    AcousticComparisonRequest, AcousticComparisonResult, BuildingMeta,
    SiteAcousticMetrics, ComparisonMetric,
};
use std::collections::HashMap;
use std::sync::Arc;

pub struct DesignComparator {
    config: Arc<crate::models::AcousticConfig>,
}

impl DesignComparator {
    pub fn new(config: Arc<crate::models::AcousticConfig>) -> Self {
        Self { config }
    }

    pub fn compare_dynasties(
        &self,
        params: &AcousticComparisonRequest,
    ) -> AcousticComparisonResult {
        let mut site_metrics = Vec::new();
        for site_id in &params.site_ids {
            if let Some(building) = self.config.ancient_buildings.get(site_id) {
                let metrics = Self::building_to_metrics(building, params.frequency, params.background_noise_db);
                site_metrics.push(metrics);
            } else if let Some(site) = self.config.sites.get(site_id) {
                let metrics = Self::site_to_metrics(site_id, site, params.frequency, params.background_noise_db);
                site_metrics.push(metrics);
            }
        }
        Self::build_comparison_result(site_metrics)
    }

    pub fn rank_by_speech_clarity(&self, building_ids: &[String]) -> Vec<(String, f64)> {
        let mut ranked: Vec<(String, f64)> = building_ids
            .iter()
            .filter_map(|id| self.config.ancient_buildings.get(id))
            .map(|b| {
                let t60 = b.typical_reverb_t60;
                let sti = (0.85 - (t60 - 1.0) * 0.20).clamp(0.2, 0.95);
                (b.building_id.clone(), sti)
            })
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        ranked
    }

    pub fn get_dynasty_design_summary(&self, dynasty: &str) -> Option<DynastyDesignSummary> {
        let buildings: Vec<&BuildingMeta> = self.config.ancient_buildings
            .values()
            .filter(|b| b.dynasty.as_deref() == Some(dynasty))
            .collect();

        if buildings.is_empty() {
            return None;
        }

        let avg_t60: f64 = buildings.iter().map(|b| b.typical_reverb_t60).sum::<f64>() / buildings.len() as f64;
        let avg_volume: f64 = buildings.iter().map(|b| b.volume_cubic_meters).sum::<f64>() / buildings.len() as f64;
        let avg_absorption: f64 = buildings.iter().map(|b| (b.wall_absorption + b.ceiling_absorption + b.floor_absorption) / 3.0).sum::<f64>() / buildings.len() as f64;

        Some(DynastyDesignSummary {
            dynasty: dynasty.to_string(),
            building_count: buildings.len(),
            avg_reverb_t60: avg_t60,
            avg_volume: avg_volume,
            avg_absorption: avg_absorption,
            typical_geometry: buildings[0].geometry_type.clone(),
            design_goal: if avg_t60 > 2.5 { "ritual_atmosphere".to_string() } else { "speech_clarity".to_string() },
        })
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DynastyDesignSummary {
    pub dynasty: String,
    pub building_count: usize,
    pub avg_reverb_t60: f64,
    pub avg_volume: f64,
    pub avg_absorption: f64,
    pub typical_geometry: String,
    pub design_goal: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BuildingMeta, Vec3, AcousticConfig};
    use std::collections::HashMap;

    fn create_test_config() -> Arc<AcousticConfig> {
        let mut config = AcousticConfig::default();
        let building = BuildingMeta {
            building_id: "test_temple".to_string(),
            name: "Test Temple".to_string(),
            category: "ancient".to_string(),
            dynasty: Some("tang".to_string()),
            dimensions: Vec3 { x: 50.0, y: 20.0, z: 50.0 },
            volume_cubic_meters: 50000.0,
            wall_absorption: 0.12,
            ceiling_absorption: 0.10,
            floor_absorption: 0.04,
            typical_reverb_t60: 3.0,
            geometry_type: "rectangular".to_string(),
            description: "Test building".to_string(),
            literature_references: vec![],
            data_quality: "estimated".to_string(),
            absorption_notes: "".to_string(),
        };
        config.ancient_buildings.insert("test_temple".to_string(), building);
        Arc::new(config)
    }

    #[test]
    fn test_design_comparator_new() {
        let config = create_test_config();
        let comparator = DesignComparator::new(config);
        assert_eq!(comparator.config.ancient_buildings.len(), 1);
    }

    #[test]
    fn test_rank_by_speech_clarity() {
        let config = create_test_config();
        let comparator = DesignComparator::new(config);
        let ids = vec!["test_temple".to_string()];
        let ranked = comparator.rank_by_speech_clarity(&ids);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0, "test_temple");
        assert!(ranked[0].1 > 0.2 && ranked[0].1 < 0.95);
    }

    #[test]
    fn test_get_dynasty_design_summary() {
        let config = create_test_config();
        let comparator = DesignComparator::new(config);
        let summary = comparator.get_dynasty_design_summary("tang");
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(s.dynasty, "tang");
        assert_eq!(s.building_count, 1);
        assert!(s.avg_reverb_t60 > 2.0);
    }

    #[test]
    fn test_building_to_metrics() {
        let config = create_test_config();
        let building = config.ancient_buildings.get("test_temple").unwrap();
        let metrics = DesignComparator::building_to_metrics(building, 1000.0, 40.0);
        assert_eq!(metrics.site_id, "test_temple");
        assert!(metrics.reverb_time_t60 > 0.5 && metrics.reverb_time_t60 < 5.0);
        assert!(metrics.sti_value >= 0.2 && metrics.sti_value <= 1.0);
    }
}
