use crate::models::{SimulatorRequest, SoundPath, SoundFieldSnapshot, SimulationParams, Vec3, AcousticConfig, SiteConfig};
use nalgebra::{Point3, Vector3, UnitVector3};
use rand::{Rng, thread_rng};
use rand_distr::{Normal, Distribution};
use num_complex::Complex;
use std::f64::consts::PI;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct SiteAcousticParams {
    pub site_id: String,
    pub site_type: String,
    pub radius: f64,
    pub wall_height: f64,
    pub wall_absorption: f64,
    pub center: Point3<f64>,
}

impl SiteAcousticParams {
    pub fn from_site_config(site_id: &str, config: &SiteConfig) -> Self {
        Self {
            site_id: site_id.to_string(),
            site_type: config.site_type.clone(),
            radius: config.radius,
            wall_height: config.wall_height,
            wall_absorption: config.wall_absorption,
            center: Point3::new(config.center[0], config.center[1], config.center[2]),
        }
    }
}

pub struct AcousticSimulator {
    pub site_params: SiteAcousticParams,
    speed_of_sound: f64,
    air_absorption_coeff: f64,
    diffraction_threshold: f64,
}

struct RayHit {
    point: Point3<f64>,
}

fn fresnel_number(path_diff: f64, frequency: f64, speed_of_sound: f64) -> f64 {
    let wavelength = speed_of_sound / frequency.max(1.0);
    2.0 * path_diff / wavelength
}

fn utd_diffraction_coeff(fresnel_n: f64) -> f64 {
    if fresnel_n < -1.0 { return 0.5; }
    if fresnel_n > 5.0 { return 0.0; }
    let x = fresnel_n * (PI / 2.0).sqrt();
    let c = 0.5 * (0.5 - x.cos() * (0.5 * PI * fresnel_n.abs()).sin().copysign(1.0));
    let s = 0.5 * (0.5 + x.sin() * (0.5 * PI * fresnel_n.abs()).cos().copysign(1.0));
    (c * c + s * s).sqrt() * 0.5
}

fn diffraction_scatter(frequency: f64, wall_height: f64, speed_of_sound: f64, diffraction_threshold: f64) -> f64 {
    let wavelength = speed_of_sound / frequency.max(1.0);
    let ratio = wavelength / wall_height.max(0.1);
    if ratio > diffraction_threshold {
        (ratio - diffraction_threshold).min(1.0) * 0.3
    } else {
        0.0
    }
}

fn compute_diffraction_loss(frequency: f64, wall_height: f64, speed_of_sound: f64) -> f64 {
    let fresnel_n = fresnel_number(wall_height * 0.5, frequency, speed_of_sound);
    let diff_coeff = utd_diffraction_coeff(fresnel_n);
    -20.0 * diff_coeff.max(0.001).log10()
}

impl AcousticSimulator {
    pub fn new(site_params: SiteAcousticParams, speed_of_sound: f64, air_absorption_coeff: f64, diffraction_threshold: f64) -> Self {
        Self {
            site_params,
            speed_of_sound,
            air_absorption_coeff,
            diffraction_threshold,
        }
    }

    pub fn trace_ray_geometric(&self, params: &SimulationParams) -> Vec<SoundPath> {
        let mut rng = thread_rng();
        let mut paths = Vec::new();
        let src = params.source_position.to_point3();
        let wavelength = self.speed_of_sound / params.frequency.max(1.0);
        let low_freq = wavelength / self.site_params.wall_height.max(0.1) > self.diffraction_threshold;

        for _ in 0..params.num_rays {
            let theta = rng.gen_range(0.0..2.0 * PI);
            let phi = rng.gen_range(0.05 * PI..0.95 * PI);
            let dir = Vector3::new(
                phi.sin() * theta.cos(),
                phi.cos(),
                phi.sin() * theta.sin(),
            );
            let dir = UnitVector3::new_normalize(dir);

            let path = self.trace_single_ray(&src, &dir, params.max_reflections, params.frequency);
            paths.push(path);

            if low_freq {
                let num_diff = rng.gen_range(1u32..=3);
                for _ in 0..num_diff {
                    let dtheta = theta + rng.gen_range(-0.3..0.3);
                    let dphi = phi + rng.gen_range(0.0..0.4 * PI);
                    let ddir = Vector3::new(
                        dphi.sin() * dtheta.cos(),
                        dphi.cos(),
                        dphi.sin() * dtheta.sin(),
                    );
                    let ddir = UnitVector3::new_normalize(ddir);
                    let mut dpath = self.trace_single_ray(&src, &ddir, params.max_reflections, params.frequency);
                    dpath.attenuation_db += compute_diffraction_loss(params.frequency, self.site_params.wall_height, self.speed_of_sound);
                    paths.push(dpath);
                }
            }
        }
        paths
    }

    fn trace_single_ray(
        &self,
        src: &Point3<f64>,
        dir: &UnitVector3<f64>,
        max_reflections: u8,
        frequency: f64,
    ) -> SoundPath {
        let mut rng = thread_rng();
        let mut current_pos = *src;
        let mut current_dir = *dir;
        let mut path_points = vec![Vec3::from_point3(&current_pos)];
        let mut total_distance = 0.0;
        let mut attenuation = 0.0;
        let mut reflections = 0u8;
        let scatter = diffraction_scatter(frequency, self.site_params.wall_height, self.speed_of_sound, self.diffraction_threshold);

        for _ in 0..=max_reflections {
            if let Some(hit) = self.intersect_circular_wall(&current_pos, &current_dir) {
                let hit_point = hit.point;
                let distance = (hit_point - current_pos).norm();
                total_distance += distance;

                attenuation += 20.0 * (total_distance / 1.0).log10().max(0.0);
                attenuation += self.air_absorption_coeff * total_distance * (frequency / 1000.0);
                attenuation += -10.0 * (1.0 - self.site_params.wall_absorption).log10();

                let fresnel_n = fresnel_number(distance * 0.5, frequency, self.speed_of_sound);
                let diff_coeff = utd_diffraction_coeff(fresnel_n);
                attenuation += -20.0 * (1.0 - diff_coeff * 0.3).max(0.001).log10();

                path_points.push(Vec3::from_point3(&hit_point));

                let normal = UnitVector3::new_normalize(hit_point - self.site_params.center);
                let reflected = current_dir.into_inner() - 2.0 * current_dir.dot(&normal) * normal.into_inner();

                let scattered = if scatter > 0.0 {
                    let perturb = Vector3::new(
                        rng.gen_range(-scatter..scatter),
                        rng.gen_range(-scatter..scatter),
                        rng.gen_range(-scatter..scatter),
                    );
                    reflected + perturb
                } else {
                    reflected
                };
                current_dir = UnitVector3::new_normalize(scattered);
                current_pos = hit_point + 0.001 * current_dir.into_inner();
                reflections += 1;
            } else {
                break;
            }
        }

        SoundPath {
            timestamp: chrono::Utc::now(),
            path_id: uuid::Uuid::new_v4(),
            site_id: self.site_params.site_id.clone(),
            source_position: Vec3::from_point3(src),
            receiver_position: Vec3::from_point3(&current_pos),
            reflection_count: reflections,
            path_points,
            travel_distance: total_distance,
            travel_time: total_distance / self.speed_of_sound,
            attenuation_db: attenuation,
            frequency,
        }
    }

    fn intersect_circular_wall(
        &self,
        origin: &Point3<f64>,
        dir: &UnitVector3<f64>,
    ) -> Option<RayHit> {
        let r = self.site_params.radius;
        let c = self.site_params.center;
        let h = self.site_params.wall_height;

        let ox = origin.x - c.x;
        let oz = origin.z - c.z;
        let dx = dir.x;
        let dz = dir.z;

        let a = dx * dx + dz * dz;
        if a.abs() < 1e-10 { return None; }

        let b = 2.0 * (ox * dx + oz * dz);
        let cc = ox * ox + oz * oz - r * r;
        let discriminant = b * b - 4.0 * a * cc;

        if discriminant < 0.0 { return None; }
        let sqrt_d = discriminant.sqrt();
        let t1 = (-b - sqrt_d) / (2.0 * a);
        let t2 = (-b + sqrt_d) / (2.0 * a);

        let t_candidates = [t1, t2];
        for &t in &t_candidates {
            if t > 0.001 {
                let hit_y = origin.y + t * dir.y;
                if hit_y >= 0.0 && hit_y <= h {
                    let point = Point3::new(
                        origin.x + t * dir.x,
                        hit_y,
                        origin.z + t * dir.z,
                    );
                    return Some(RayHit { point });
                }
            }
        }
        None
    }

    pub fn compute_wave_sound_field(
        &self,
        params: &SimulationParams,
        grid_res: u16,
    ) -> SoundFieldSnapshot {
        let nx = grid_res;
        let ny = grid_res;
        let r = self.site_params.radius + 2.0;
        let c = self.site_params.center;
        let src = params.source_position.to_point3();
        let k = 2.0 * PI * params.frequency / self.speed_of_sound;

        let mut field = vec![vec![0.0f64; ny as usize]; nx as usize];

        for i in 0..nx as usize {
            for j in 0..ny as usize {
                let px = c.x - r + 2.0 * r * (i as f64) / (nx as f64 - 1.0);
                let pz = c.z - r + 2.0 * r * (j as f64) / (ny as f64 - 1.0);
                let py = 1.8;
                let receiver = Point3::new(px, py, pz);

                let direct_dist = (receiver - src).norm();
                let mut pressure = Complex::new(0.0, 0.0);
                if direct_dist > 0.1 {
                    pressure += Complex::new(0.0, -k).exp() / direct_dist;
                }

                for img_idx in 0..20 {
                    let angle = 2.0 * PI * (img_idx as f64 + 1.0) / 20.0;
                    let img_src = Point3::new(
                        c.x + r * angle.cos(),
                        src.y,
                        c.z + r * angle.sin(),
                    );
                    let img_dist = (receiver - img_src).norm();
                    if img_dist > 0.1 {
                        let attn = (1.0 - self.site_params.wall_absorption).powi(img_idx as i32 + 1);
                        pressure += attn * Complex::new(0.0, -k * img_dist).exp() / img_dist;
                    }
                }

                let pressure_norm = pressure.norm();
                field[i][j] = if pressure_norm > 0.0 {
                    20.0 * pressure_norm.log10()
                } else {
                    -100.0
                };
            }
        }

        let min_val = field.iter().flatten().cloned().fold(f64::INFINITY, f64::min);
        let max_val = field.iter().flatten().cloned().fold(f64::NEG_INFINITY, f64::max);
        if max_val > min_val {
            for row in field.iter_mut() {
                for v in row.iter_mut() {
                    *v = (*v - min_val) / (max_val - min_val);
                }
            }
        }

        SoundFieldSnapshot {
            timestamp: chrono::Utc::now(),
            snapshot_id: uuid::Uuid::new_v4(),
            site_id: self.site_params.site_id.clone(),
            grid_resolution: grid_res,
            grid_points_x: nx,
            grid_points_y: ny,
            pressure_field: field,
            frequency: params.frequency,
            source_position: params.source_position.clone(),
        }
    }

    pub fn generate_impulse_response(&self, params: &SimulationParams, duration_sec: f64, sample_rate: u32) -> Vec<f64> {
        let num_samples = (duration_sec * sample_rate as f64) as usize;
        let mut ir = vec![0.0f64; num_samples];
        let paths = self.trace_ray_geometric(params);
        let mut rng = thread_rng();
        let normal = Normal::new(0.0, 0.01).unwrap();

        for path in &paths {
            let sample_idx = (path.travel_time * sample_rate as f64) as usize;
            if sample_idx < num_samples {
                let amplitude = (-path.attenuation_db / 20.0).exp();
                ir[sample_idx] += amplitude;
            }
        }

        for i in 0..num_samples {
            ir[i] += normal.sample(&mut rng);
            let t_decay = (i as f64) / sample_rate as f64;
            ir[i] *= (-3.0 * t_decay).exp();
        }

        let peak = ir.iter().cloned().fold(0.0f64, f64::max);
        if peak > 0.0 {
            for v in ir.iter_mut() { *v /= peak; }
        }
        ir
    }

    pub fn compute_reverb_from_ir(ir: &[f64], sample_rate: u32) -> (f64, f64, f64) {
        let mut energy: Vec<f64> = ir.iter().map(|x| x * x).collect();
        let total_energy: f64 = energy.iter().sum();
        if total_energy < 1e-10 { return (2.0, 1.8, 1.5); }

        let mut cumulative = 0.0f64;
        let mut t_edt = 0.0;
        let mut t_t30 = 0.0;
        let mut t_t60 = 0.0;
        let mut found_edt = false;
        let mut found_t30 = false;
        let mut found_t60 = false;

        for (i, &e) in energy.iter().enumerate() {
            cumulative += e;
            let decay_db = 10.0 * ((total_energy - cumulative) / total_energy).log10();
            let t = i as f64 / sample_rate as f64;

            if !found_edt && decay_db <= -10.0 { t_edt = t * 6.0; found_edt = true; }
            if !found_t30 && decay_db <= -30.0 { t_t30 = t * 2.0; found_t30 = true; }
            if !found_t60 && decay_db <= -60.0 { t_t60 = t; found_t60 = true; }
        }

        if !found_edt { t_edt = 1.8; }
        if !found_t30 { t_t30 = t_t60 * 0.9; }
        if !found_t60 { t_t60 = t_t30 / 0.9; }

        (t_t60.max(0.5), t_t30.max(0.4), t_edt.max(0.3))
    }
}

pub struct SimulatorTask {
    config: Arc<AcousticConfig>,
    rx: mpsc::Receiver<SimulatorRequest>,
}

impl SimulatorTask {
    pub fn new(config: Arc<AcousticConfig>, rx: mpsc::Receiver<SimulatorRequest>) -> Self {
        Self { config, rx }
    }

    pub async fn run(&mut self) {
        while let Some(req) = self.rx.recv().await {
            match req {
                SimulatorRequest::TraceRays { params, reply } => {
                    let result = self.handle_trace_rays(&params);
                    let _ = reply.send(result);
                }
                SimulatorRequest::WaveField { params, reply } => {
                    let result = self.handle_wave_field(&params);
                    let _ = reply.send(result);
                }
                SimulatorRequest::VirtualExperience { params, reply } => {
                    let result = self.handle_virtual_experience(&params);
                    let _ = reply.send(result);
                }
                SimulatorRequest::NoiseSimulation { params, reply } => {
                    let result = self.handle_noise_simulation(&params);
                    let _ = reply.send(result);
                }
            }
        }
    }

    fn handle_trace_rays(&self, params: &SimulationParams) -> Vec<SoundPath> {
        if let Some(sim) = self.create_simulator(&params.site_id) {
            sim.trace_ray_geometric(params)
        } else {
            Vec::new()
        }
    }

    fn handle_wave_field(&self, params: &SimulationParams) -> SoundFieldSnapshot {
        if let Some(sim) = self.create_simulator(&params.site_id) {
            let grid_res = self.config.simulation_defaults.wave_field_grid_resolution;
            sim.compute_wave_sound_field(params, grid_res)
        } else {
            SoundFieldSnapshot {
                timestamp: chrono::Utc::now(),
                snapshot_id: uuid::Uuid::new_v4(),
                site_id: params.site_id.clone(),
                grid_resolution: 0,
                grid_points_x: 0,
                grid_points_y: 0,
                pressure_field: Vec::new(),
                frequency: params.frequency,
                source_position: params.source_position.clone(),
            }
        }
    }

    fn create_simulator(&self, site_id: &str) -> Option<AcousticSimulator> {
        let site_config = self.config.sites.get(site_id)?;
        let site_params = SiteAcousticParams::from_site_config(site_id, site_config);
        Some(AcousticSimulator::new(
            site_params,
            self.config.simulation_defaults.speed_of_sound,
            self.config.simulation_defaults.air_absorption_coefficient,
            self.config.simulation_defaults.diffraction_threshold,
        ))
    }

    fn create_building_simulator(&self, building_id: &str) -> Option<BuildingAcousticSimulator> {
        let building = self.config.ancient_buildings.get(building_id)
            .or_else(|| self.config.concert_halls.get(building_id))?;
        Some(BuildingAcousticSimulator::new(
            building.clone(),
            self.config.simulation_defaults.speed_of_sound,
            self.config.simulation_defaults.air_absorption_coefficient,
        ))
    }

    fn handle_virtual_experience(&self, params: &crate::models::VirtualExperienceRequest) -> crate::models::VirtualExperienceResult {
        use crate::models::{BinauralImpulseResponse, Vec3};

        let sim = self.create_simulator(&params.site_id)
            .or_else(|| {
                let _building = self.config.ancient_buildings.get(&params.site_id)
                    .or_else(|| self.config.concert_halls.get(&params.site_id));
                None
            });

        let sim_params = crate::models::SimulationParams {
            site_id: params.site_id.clone(),
            source_position: params.source_position.clone(),
            frequency: params.frequency,
            max_reflections: 12,
            num_rays: 200,
            temperature: 20.0,
            humidity: 50.0,
        };

        let paths = if let Some(sim) = &sim {
            sim.trace_ray_geometric(&sim_params)
        } else {
            Vec::new()
        };

        let src = params.source_position.to_point3();
        let listener = params.listener_position.to_point3();
        let direct_dist = (listener - src).norm();

        let direct_path = crate::models::SoundPath {
            timestamp: chrono::Utc::now(),
            path_id: uuid::Uuid::new_v4(),
            site_id: params.site_id.clone(),
            source_position: params.source_position.clone(),
            receiver_position: params.listener_position.clone(),
            reflection_count: 0,
            path_points: vec![params.source_position.clone(), params.listener_position.clone()],
            travel_distance: direct_dist,
            travel_time: direct_dist / 343.0,
            attenuation_db: 20.0 * (direct_dist / 1.0).log10().max(0.0),
            frequency: params.frequency,
        };

        let reflection_paths: Vec<crate::models::SoundPath> = paths.iter()
            .filter(|p| p.reflection_count >= 1)
            .cloned()
            .collect();

        let total_paths = paths.clone();

        let ir = if let Some(sim) = &sim {
            sim.generate_impulse_response(&sim_params, 2.0, 44100)
        } else {
            vec![0.0; 88200]
        };

        let (t60, _, _) = AcousticSimulator::compute_reverb_from_ir(&ir, 44100);

        let binaural_ir = Self::compute_binaural_ir(
            &params.source_position,
            &params.listener_position,
            &ir,
            44100,
        );

        let echo_count = reflection_paths.len().min(10) as u32;
        let mut sorted_reflections = reflection_paths.clone();
        sorted_reflections.sort_by(|a, b| a.travel_time.partial_cmp(&b.travel_time).unwrap());

        let echo_delay_1 = sorted_reflections.get(0).map(|p| p.travel_time).unwrap_or(0.0);
        let echo_delay_2 = sorted_reflections.get(1).map(|p| p.travel_time).unwrap_or(0.0);
        let echo_delay_3 = sorted_reflections.get(2).map(|p| p.travel_time).unwrap_or(0.0);

        let base_sti = match params.site_id.as_str() {
            "huiyinbi" => 0.72,
            "sanyinshi" => 0.65,
            "huanqiutan" => 0.68,
            _ => 0.60,
        };

        let noise_reduction = if params.include_noise {
            0.15
        } else {
            0.0
        };

        let sti_without_noise = base_sti;
        let sti_with_noise = (base_sti - noise_reduction).max(0.0);

        let speech_intelligibility = crate::models::SpeechIntelligibility {
            timestamp: chrono::Utc::now(),
            analysis_id: uuid::Uuid::new_v4(),
            site_id: params.site_id.clone(),
            site_name: match params.site_id.as_str() {
                "huiyinbi" => "回音壁".to_string(),
                "sanyinshi" => "三音石".to_string(),
                "huanqiutan" => "圜丘坛".to_string(),
                id => id.to_string(),
            },
            sti_value: sti_with_noise,
            rasti_value: sti_with_noise * 1.05,
            crispness: 5.0,
            definition_d50: 50.0 + (sti_with_noise - 0.5) * 100.0,
            clarity_c50: 3.0 + (sti_with_noise - 0.5) * 30.0,
            center_time: 0.05,
            frequency_bands: vec![125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0],
            band_snr: vec![15.0, 18.0, 22.0, 20.0, 18.0, 15.0, 12.0],
            speech_content: params.speech_text.clone(),
        };

        let sound_preservation_score = if params.site_id == "huiyinbi" {
            0.92
        } else if params.site_id == "sanyinshi" {
            0.85
        } else {
            0.78
        };

        crate::models::VirtualExperienceResult {
            site_id: params.site_id.clone(),
            source_position: params.source_position.clone(),
            listener_position: params.listener_position.clone(),
            direct_path,
            reflection_paths,
            total_paths,
            impulse_response: ir,
            binaural_ir,
            sti_with_noise,
            sti_without_noise,
            speech_intelligibility,
            echo_count,
            echo_delay_1,
            echo_delay_2,
            echo_delay_3,
            reverberation_time_t60: t60,
            sound_preservation_score,
        }
    }

    fn compute_binaural_ir(
        src_pos: &crate::models::Vec3,
        listener_pos: &crate::models::Vec3,
        mono_ir: &[f64],
        sample_rate: u32,
    ) -> crate::models::BinauralImpulseResponse {
        use std::f64::consts::PI;
        let head_radius = 0.0875;
        let src = src_pos.to_point3();
        let listener = listener_pos.to_point3();
        let sound_direction = (src - listener).normalize();
        let azimuth = sound_direction.x.atan2(sound_direction.z);
        let elevation = sound_direction.y.asin();

        // Woodworth公式: ITD = (a/c) * (sin(θ) + θ)  for |θ| <= π/2
        // 比纯几何投影更精确，高频极限下退化为Woodworth近似
        let abs_az = azimuth.abs().min(PI / 2.0);
        let itd_woodworth = (head_radius / 343.0) * (abs_az.sin() + abs_az);
        let itd = itd_woodworth.copysign(azimuth);

        // 频率依赖ILD模型
        // 低频(<500Hz): ILD ≈ 0-2dB (声波绕射)
        // 中频(1-2kHz): ILD ≈ 3-8dB (头影开始显著)
        // 高频(>4kHz): ILD ≈ 8-15dB (头影完全遮挡)
        // 使用典型1kHz作为参考频段
        let ild_low = 1.5 * azimuth.sin().abs();
        let ild_mid = 6.0 * azimuth.sin().abs();
        let ild_high = 12.0 * azimuth.sin().abs();
        let ild = ild_mid;

        // 耳廓效应 (Pinna effect): 高频方向性增益
        // 前方声音得到增强，后方得到衰减，特别是4-8kHz频段
        let pinna_gain_front = 2.0;
        let pinna_gain_back = -3.0;
        let pinna_gain_db = if azimuth.abs() < PI / 3.0 {
            pinna_gain_front * (1.0 - azimuth.abs() / (PI / 3.0))
        } else {
            pinna_gain_back * ((azimuth.abs() - PI / 3.0) / (2.0 * PI / 3.0))
        };

        // 肩部反射效应: 约4-6kHz的谐振，增强前方定位
        let shoulder_reflection_delay = 0.000_300; // 300μs
        let shoulder_reflection_gain = 0.15;

        let delay_samples = (itd.abs() * sample_rate as f64).round() as usize;
        let shoulder_delay_samples = (shoulder_reflection_delay * sample_rate as f64).round() as usize;

        let contralateral_atten = 10.0_f64.powf(-ild / 20.0);
        let pinna_linear = 10.0_f64.powf(pinna_gain_db / 20.0);

        let mut left_ir = Vec::with_capacity(mono_ir.len());
        let mut right_ir = Vec::with_capacity(mono_ir.len());

        if itd >= 0.0 {
            for i in 0..mono_ir.len() {
                let left_val = if i < delay_samples { 0.0 } else { mono_ir[i - delay_samples] };
                let mut right_val = mono_ir[i] * contralateral_atten;

                if i >= shoulder_delay_samples {
                    right_val += mono_ir[i - shoulder_delay_samples] * shoulder_reflection_gain * contralateral_atten;
                }

                left_ir.push(left_val * pinna_linear);
                right_ir.push(right_val);
            }
        } else {
            for i in 0..mono_ir.len() {
                let mut left_val = mono_ir[i] * contralateral_atten;
                let right_val = if i < delay_samples { 0.0 } else { mono_ir[i - delay_samples] };

                if i >= shoulder_delay_samples {
                    left_val += mono_ir[i - shoulder_delay_samples] * shoulder_reflection_gain * contralateral_atten;
                }

                left_ir.push(left_val);
                right_ir.push(right_val * pinna_linear);
            }
        }

        crate::models::BinauralImpulseResponse {
            left_ear: left_ir,
            right_ear: right_ir,
            sample_rate,
            listener_position: listener_pos.clone(),
            source_position: src_pos.clone(),
            itd_seconds: itd,
            ild_db: ild,
            azimuth_rad: azimuth,
            playback_mode: "headphone".to_string(),
            headphone_optimized: true,
            hrtf_notes: format!(
                "Woodworth ITD: {:.1}μs | ILD(band): low={:.1}dB mid={:.1}dB high={:.1}dB | pinna: {:+.1}dB | shoulder_refl: {:.0}μs",
                itd * 1e6, ild_low, ild_mid, ild_high, pinna_gain_db, shoulder_reflection_delay * 1e6
            ),
        }
    }

    fn handle_noise_simulation(&self, params: &crate::models::NoiseSimulationRequest) -> crate::models::NoiseSimulationResult {
        use std::collections::HashMap;

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
        let occlusion_factor = if n_sources > 10.0 {
            1.0 - 0.03 * (n_sources / 10.0).ln().min(1.5)
        } else {
            1.0
        };

        let mut total_energy_at_listener = 0.0f64;
        for noise_src in &params.noise_sources {
            let src = noise_src.position.to_point3();
            let diff = listener - src;
            let dist = diff.norm().max(1.0);

            let mut level_at_listener = noise_src.sound_level_db - 20.0 * (dist / 1.0).log10();

            if let Some(ref dir) = noise_src.direction {
                let dir_vec = dir.to_point3();
                let dir_norm = dir_vec.norm();
                if dir_norm > 1e-6 && dist > 1.0 {
                    let to_listener = diff.normalize();
                    let src_dir = (dir_vec / dir_norm).into();
                    let cos_angle: f64 = to_listener.x * src_dir.x + to_listener.y * src_dir.y + to_listener.z * src_dir.z;
                    let di = noise_src.directivity_index;
                    let directivity_gain = 10.0_f64.powf(di / 10.0);
                    let q = directivity_gain * cos_angle.max(0.0).powf(directivity_gain.ln().max(1.0)) + 1.0;
                    level_at_listener += 10.0 * (q / (4.0 * PI)).log10();
                }
            }

            level_at_listener += 10.0 * occlusion_factor.log10();

            total_energy_at_listener += 10.0_f64.powf(level_at_listener / 10.0);
        }

        let total_noise_level_db = 10.0 * total_energy_at_listener.log10();
        let snr_db = params.speech_level_db - total_noise_level_db;

        let clean_sti = self.compute_clean_sti_for_site(&params.site_id);
        let snr_factor = (snr_db / 20.0).tanh();
        let noisy_sti = 0.2 + (clean_sti - 0.2) * snr_factor.max(0.0);
        let sti_degradation = clean_sti - noisy_sti;

        let site_area = 3.14159 * 30.75 * 30.75;
        let noise_per_person = 55.0;
        let crowd_density = 0.3;
        let max_visitors = ((site_area * crowd_density) as u32).min(500);

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
                    let mut level = noise_src.sound_level_db - 20.0 * (dist / 1.0).log10();

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

        crate::models::NoiseSimulationResult {
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

    fn compute_clean_sti_for_site(&self, site_id: &str) -> f64 {
        match site_id {
            "huiyinbi" => 0.75,
            "sanyinshi" => 0.70,
            "huanqiutan" => 0.68,
            id if id.contains("temple") => {
                if let Some(building) = self.config.ancient_buildings.get(id) {
                    let t60 = building.typical_reverb_t60;
                    (0.85 - (t60 - 1.0) * 0.20).clamp(0.2, 0.95)
                } else {
                    0.65
                }
            }
            id if id.contains("hall") => {
                if let Some(hall) = self.config.concert_halls.get(id) {
                    let t60 = hall.typical_reverb_t60;
                    (0.85 - (t60 - 1.0) * 0.20).clamp(0.2, 0.95)
                } else {
                    0.70
                }
            }
            _ => 0.65,
        }
    }
}

pub struct BuildingAcousticSimulator {
    building: crate::models::BuildingMeta,
    speed_of_sound: f64,
    air_absorption_coeff: f64,
}

impl BuildingAcousticSimulator {
    pub fn new(building: crate::models::BuildingMeta, speed_of_sound: f64, air_absorption_coeff: f64) -> Self {
        Self {
            building,
            speed_of_sound,
            air_absorption_coeff,
        }
    }

    pub fn compute_acoustic_metrics(&self) -> crate::models::SiteAcousticMetrics {
        let b = &self.building;
        let volume = b.volume_cubic_meters;
        let surface_area = 2.0 * (b.dimensions.x * b.dimensions.z + b.dimensions.x * b.dimensions.y + b.dimensions.z * b.dimensions.y);
        let avg_absorption = (b.wall_absorption + b.ceiling_absorption + b.floor_absorption) / 3.0;

        let t60 = if avg_absorption > 0.001 {
            0.161 * volume / (surface_area * avg_absorption)
        } else {
            b.typical_reverb_t60
        };

        let t60 = t60.max(0.5).min(5.0);

        let bass_ratio = 1.2 - avg_absorption * 2.0;
        let brilliance = 0.8 + avg_absorption;
        let intimacy = if volume < 10000.0 { 0.9 } else if volume < 30000.0 { 0.7 } else { 0.5 };
        let warmth = 0.5 + bass_ratio * 0.3;
        let loudness = 1.0 - t60 / 5.0;
        let echo_strength = if b.geometry_type.contains("circular") { 0.9 } else { 0.5 };

        let c50 = 3.0 + (1.5 - t60) * 5.0;
        let d50 = 50.0 + (1.5 - t60) * 20.0;
        let sti = 0.4 + (1.5 - t60) * 0.3;
        let sti = sti.clamp(0.2, 0.95);

        crate::models::SiteAcousticMetrics {
            site_id: b.building_id.clone(),
            site_name: b.name.clone(),
            category: b.category.clone(),
            dynasty: b.dynasty.clone(),
            reverb_time_t60: t60,
            reverb_time_edt: t60 * 0.8,
            clarity_c50: c50.clamp(-5.0, 20.0),
            definition_d50: d50.clamp(10.0, 90.0),
            sti_value: sti,
            rasti_value: sti * 1.05,
            sound_pressure_level: 70.0 + loudness * 20.0,
            center_time: t60 * 0.3,
            bass_ratio: bass_ratio.clamp(0.5, 2.0),
            brilliance: brilliance.clamp(0.0, 1.0),
            intimacy: intimacy.clamp(0.0, 1.0),
            warmth: warmth.clamp(0.0, 1.0),
            loudness: loudness.clamp(0.0, 1.0),
            echo_strength: echo_strength.clamp(0.0, 1.0),
            description: b.description.clone(),
        }
    }
}
