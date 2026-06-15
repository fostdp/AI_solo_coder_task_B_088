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
}
