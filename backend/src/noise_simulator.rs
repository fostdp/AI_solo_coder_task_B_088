use crate::models::{
    NoiseSimulationRequest, NoiseSimulationResult, NoiseSource,
    Vec3, AcousticConfig,
};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::Arc;

pub struct NoiseSimulator {
    config: Arc<AcousticConfig>,
}

impl NoiseSimulator {
    pub fn new(config: Arc<AcousticConfig>) -> Self {
        Self { config }
    }

    pub fn simulate(&self, params: &NoiseSimulationRequest) -> NoiseSimulationResult {
        let grid_res = 32u16;
        let mut noise_map = vec![vec![0.0f64; grid_res as usize]; grid_res as usize];

        let mut total_noise_energy = 0.0f64;
        let mut noise_contribution = HashMap::new();
        let mut group_energy: HashMap<String, f64> = HashMap::new();

        for (i, noise_src) in params.noise_sources.iter().enumerate() {
            let noise_power = 10.0_f64.powf(noise_src.sound_level_db / 10.0);
            total_noise_energy += noise_power;
            let gid = noise_src.group_id.clone().unwrap_or_else(|| format!("noise_{}", i));
            *group_energy.entry(gid.clone()).or_insert(0.0) += noise_power;
            noise_contribution.insert(gid, noise_src.sound_level_db);
        }

        let listener = params.listener_position.to_point3();

        let n_sources = params.noise_sources.len().max(1) as f64;
        let occlusion_factor = Self::compute_crowd_occlusion(n_sources);

        let mut total_energy_at_listener = 0.0f64;
        for noise_src in &params.noise_sources {
            let src = noise_src.position.to_point3();
            let diff = listener - src;
            let dist = diff.norm().max(1.0);

            let mut level_at_listener = Self::compute_spherical_attenuation(noise_src.sound_level_db, dist);
            level_at_listener += Self::compute_directivity_gain(noise_src, diff, dist);
            level_at_listener += 10.0 * occlusion_factor.log10();

            total_energy_at_listener += 10.0_f64.powf(level_at_listener / 10.0);
        }

        let total_noise_level_db = 10.0 * total_energy_at_listener.log10();
        let snr_db = params.speech_level_db - total_noise_level_db;

        let clean_sti = Self::compute_clean_sti_for_site(&params.site_id, &self.config);
        let snr_factor = (snr_db / 20.0).tanh();
        let noisy_sti = 0.2 + (clean_sti - 0.2) * snr_factor.max(0.0);
        let sti_degradation = clean_sti - noisy_sti;

        let site_area = PI * 30.75 * 30.75;
        let crowd_density = 0.3;
        let max_visitors = ((site_area * crowd_density) as u32).min(500);

        noise_map = Self::generate_noise_heatmap(params, grid_res, occlusion_factor);

        NoiseSimulationResult {
            site_id: params.site_id.clone(),
            total_noise_level_db,
            speech_level_db: params.speech_level_db,
            snr_db,
            sti_clean: clean_sti,
            sti_noisy: noisy_sti.max(0.0),
            sti_degradation: sti_degradation.max(0.0),
            noise_contribution,
            recommended_max_visitors: max_visitors,
            crowd_noise_map: noise_map,
            grid_resolution: grid_res,
        }
    }

    pub fn simulate_single_source(
        source: &NoiseSource,
        listener_pos: &Vec3,
    ) -> f64 {
        let src = source.position.to_point3();
        let listener = listener_pos.to_point3();
        let diff = listener - src;
        let dist = diff.norm().max(1.0);

        let mut level = Self::compute_spherical_attenuation(source.sound_level_db, dist);
        level += Self::compute_directivity_gain(source, diff, dist);
        level
    }

    pub fn compute_crowd_occlusion(n_sources: f64) -> f64 {
        if n_sources <= 10.0 {
            1.0
        } else {
            1.0 - 0.03 * (n_sources / 10.0).ln().min(1.5)
        }
    }

    pub fn compute_spherical_attenuation(source_level_db: f64, distance_m: f64) -> f64 {
        source_level_db - 20.0 * (distance_m / 1.0).log10()
    }

    pub fn compute_directivity_gain(source: &NoiseSource, to_listener: nalgebra::Vector3<f64>, dist: f64) -> f64 {
        if let Some(ref dir) = source.direction {
            let dir_vec = dir.to_point3();
            let dir_norm = dir_vec.norm();
            if dir_norm > 1e-6 && dist > 1.0 {
                let to_listener_n = to_listener.normalize();
                let src_dir = (dir_vec / dir_norm).into();
                let cos_angle: f64 = to_listener_n.x * src_dir.x + to_listener_n.y * src_dir.y + to_listener_n.z * src_dir.z;
                let di = source.directivity_index;
                let directivity_gain = 10.0_f64.powf(di / 10.0);
                let q = directivity_gain * cos_angle.max(0.0).powf(directivity_gain.ln().max(1.0)) + 1.0;
                return 10.0 * (q / (4.0 * PI)).log10();
            }
        }
        0.0
    }

    pub fn compute_clean_sti_for_site(site_id: &str, config: &AcousticConfig) -> f64 {
        match site_id {
            "huiyinbi" => 0.75,
            "sanyinshi" => 0.70,
            "huanqiutan" => 0.68,
            id if id.contains("temple") => {
                if let Some(building) = config.ancient_buildings.get(id) {
                    let t60 = building.typical_reverb_t60;
                    (0.85 - (t60 - 1.0) * 0.20).clamp(0.2, 0.95)
                } else {
                    0.65
                }
            }
            id if id.contains("hall") => {
                if let Some(hall) = config.concert_halls.get(id) {
                    let t60 = hall.typical_reverb_t60;
                    (0.85 - (t60 - 1.0) * 0.20).clamp(0.2, 0.95)
                } else {
                    0.70
                }
            }
            _ => 0.65,
        }
    }

    pub fn compute_sti_degradation(clean_sti: f64, snr_db: f64) -> (f64, f64) {
        let snr_factor = (snr_db / 20.0).tanh();
        let noisy_sti = 0.2 + (clean_sti - 0.2) * snr_factor.max(0.0);
        let degradation = clean_sti - noisy_sti;
        (noisy_sti.max(0.0), degradation.max(0.0))
    }

    pub fn estimate_noise_from_visitor_count(visitor_count: u32, per_person_level_db: f64) -> f64 {
        if visitor_count == 0 {
            return 0.0;
        }
        let per_person_energy = 10.0_f64.powf(per_person_level_db / 10.0);
        let total_energy = visitor_count as f64 * per_person_energy;
        10.0 * total_energy.log10()
    }

    fn generate_noise_heatmap(
        params: &NoiseSimulationRequest,
        grid_res: u16,
        occlusion_factor: f64,
    ) -> Vec<Vec<f64>> {
        let mut noise_map = vec![vec![0.0f64; grid_res as usize]; grid_res as usize];
        let grid_scale = 61.5 / grid_res as f64;

        for i in 0..grid_res as usize {
            for j in 0..grid_res as usize {
                let px = -30.75 + i as f64 * grid_scale;
                let pz = -30.75 + j as f64 * grid_scale;

                let mut point_energy = 0.0f64;
                for noise_src in &params.noise_sources {
                    let dx = px - noise_src.position.x;
                    let dz = pz - noise_src.position.z;
                    let dist = (dx * dx + dz * dz).sqrt().max(0.5);
                    let mut level = Self::compute_spherical_attenuation(noise_src.sound_level_db, dist);

                    if let Some(ref dir) = noise_src.direction {
                        let dir_vec = dir.to_point3();
                        let dir_norm = dir_vec.norm();
                        if dir_norm > 1e-6 {
                            let dx_n = dx / dist;
                            let dz_n = dz / dist;
                            let cos_a = (dx_n * dir_vec.x + dz_n * dir_vec.z) / dir_norm;
                            level += noise_src.directivity_index * cos_a.max(0.0);
                        }
                    }

                    level += 10.0 * occlusion_factor.log10();
                    point_energy += 10.0_f64.powf(level / 10.0);
                }

                noise_map[i][j] = if point_energy > 0.0 {
                    10.0 * point_energy.log10()
                } else {
                    0.0
                };
            }
        }

        let min_val = noise_map.iter().flatten().cloned().fold(f64::INFINITY, f64::min);
        let max_val = noise_map.iter().flatten().cloned().fold(f64::NEG_INFINITY, f64::max);
        if max_val > min_val {
            for row in noise_map.iter_mut() {
                for v in row.iter_mut() {
                    *v = (*v - min_val) / (max_val - min_val);
                }
            }
        }

        noise_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AcousticConfig, NoiseSource, Vec3};

    #[test]
    fn test_noise_simulator_new() {
        let config = Arc::new(AcousticConfig::default());
        let sim = NoiseSimulator::new(config);
        assert_eq!(sim.config.ancient_buildings.len(), 0);
    }

    #[test]
    fn test_compute_spherical_attenuation() {
        let level = NoiseSimulator::compute_spherical_attenuation(70.0, 10.0);
        assert!((level - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_compute_crowd_occlusion() {
        assert_eq!(NoiseSimulator::compute_crowd_occlusion(5.0), 1.0);
        let occ = NoiseSimulator::compute_crowd_occlusion(20.0);
        assert!(occ < 1.0 && occ > 0.9);
    }

    #[test]
    fn test_compute_sti_degradation() {
        let (noisy, degrad) = NoiseSimulator::compute_sti_degradation(0.75, 10.0);
        assert!(noisy < 0.75);
        assert!(degrad > 0.0);
    }

    #[test]
    fn test_estimate_noise_from_visitor_count() {
        let level = NoiseSimulator::estimate_noise_from_visitor_count(0, 55.0);
        assert_eq!(level, 0.0);
        let level_10 = NoiseSimulator::estimate_noise_from_visitor_count(10, 55.0);
        let level_100 = NoiseSimulator::estimate_noise_from_visitor_count(100, 55.0);
        assert!(level_100 > level_10);
        assert!(level_100 - level_10 < 15.0);
    }

    #[test]
    fn test_compute_clean_sti_for_site() {
        let config = AcousticConfig::default();
        let sti_huiyinbi = NoiseSimulator::compute_clean_sti_for_site("huiyinbi", &config);
        assert_eq!(sti_huiyinbi, 0.75);

        let sti_unknown = NoiseSimulator::compute_clean_sti_for_site("unknown", &config);
        assert_eq!(sti_unknown, 0.65);
    }

    #[test]
    fn test_compute_directivity_gain_no_direction() {
        let source = NoiseSource {
            position: Vec3 { x: 0.0, y: 1.5, z: 0.0 },
            sound_level_db: 65.0,
            source_type: "voice".to_string(),
            frequency_hz: 1000.0,
            direction: None,
            directivity_index: 3.0,
            group_id: None,
        };
        let to_listener = nalgebra::Vector3::new(5.0, 0.0, 0.0);
        let gain = NoiseSimulator::compute_directivity_gain(&source, to_listener, 5.0);
        assert_eq!(gain, 0.0);
    }

    #[test]
    fn test_simulate_single_source() {
        let source = NoiseSource {
            position: Vec3 { x: 0.0, y: 1.5, z: 0.0 },
            sound_level_db: 70.0,
            source_type: "voice".to_string(),
            frequency_hz: 1000.0,
            direction: None,
            directivity_index: 3.0,
            group_id: None,
        };
        let listener = Vec3 { x: 10.0, y: 1.5, z: 0.0 };
        let level = NoiseSimulator::simulate_single_source(&source, &listener);
        assert!(level > 0.0 && level < 70.0);
    }

    #[test]
    fn test_simulate_noise_empty_sources() {
        let config = Arc::new(AcousticConfig::default());
        let sim = NoiseSimulator::new(config);
        let params = NoiseSimulationRequest {
            site_id: "huiyinbi".to_string(),
            speech_level_db: 70.0,
            noise_sources: vec![],
            listener_position: Vec3 { x: 0.0, y: 1.5, z: 0.0 },
            background_noise_db: 30.0,
            distribution_mode: "uniform".to_string(),
            visitor_count: 0,
        };
        let result = sim.simulate(&params);
        assert_eq!(result.total_noise_level_db, f64::NEG_INFINITY);
    }

    #[test]
    fn test_generate_noise_heatmap_dimensions() {
        let config = AcousticConfig::default();
        let source = NoiseSource {
            position: Vec3 { x: 5.0, y: 1.5, z: 0.0 },
            sound_level_db: 65.0,
            source_type: "voice".to_string(),
            frequency_hz: 1000.0,
            direction: None,
            directivity_index: 3.0,
            group_id: None,
        };
        let params = NoiseSimulationRequest {
            site_id: "huiyinbi".to_string(),
            speech_level_db: 70.0,
            noise_sources: vec![source],
            listener_position: Vec3 { x: 0.0, y: 1.5, z: 0.0 },
            background_noise_db: 30.0,
            distribution_mode: "uniform".to_string(),
            visitor_count: 1,
        };
        let heatmap = NoiseSimulator::generate_noise_heatmap(&params, 32, 1.0);
        assert_eq!(heatmap.len(), 32);
        assert_eq!(heatmap[0].len(), 32);
    }
}
