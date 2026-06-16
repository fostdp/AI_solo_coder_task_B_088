use crate::models::{
    AcousticConfig, SimulationParams, SoundPath, SoundFieldSnapshot,
    NoiseSimulationRequest, NoiseSimulationResult,
    VirtualExperienceRequest, VirtualExperienceResult,
    AcousticComparisonRequest, AcousticComparisonResult,
    Vec3,
};
use crate::acoustic_simulator::{AcousticSimulator, SiteAcousticParams};
use crate::noise_simulator::NoiseSimulator;
use crate::vr_echo_wall::VrEchoWallSimulator;
use crate::design_comparator::DesignComparator;
use crate::era_comparator::EraComparator;
use std::sync::Arc;
use std::time::Instant;
use metrics::{histogram, counter};

pub struct AcousticComputePool {
    config: Arc<AcousticConfig>,
    thread_pool: rayon::ThreadPool,
}

impl AcousticComputePool {
    pub fn new(config: Arc<AcousticConfig>) -> Self {
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);

        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("acoustic-compute-{}", i))
            .build()
            .expect("Failed to build acoustic compute thread pool");

        Self { config, thread_pool }
    }

    pub fn with_num_threads(config: Arc<AcousticConfig>, num_threads: usize) -> Self {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("acoustic-compute-{}", i))
            .build()
            .expect("Failed to build acoustic compute thread pool");

        Self { config, thread_pool }
    }

    pub async fn trace_rays(&self, params: SimulationParams) -> Result<Vec<SoundPath>, String> {
        let config = self.config.clone();
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let site_id = params.site_id.clone();
            let simulator = Self::create_simulator(&config, &site_id)
                .ok_or_else(|| format!("Invalid site_id: {}", site_id))?;
            Ok(simulator.trace_ray_geometric(&params))
        }).await;

        histogram!("acoustic_compute_duration_seconds", "task" => "trace_rays")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "trace_rays").increment(1);

        result
    }

    pub async fn compute_wave_field(&self, params: SimulationParams) -> Result<SoundFieldSnapshot, String> {
        let config = self.config.clone();
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let site_id = params.site_id.clone();
            let grid_res = config.simulation_defaults.wave_field_grid_resolution;
            let simulator = Self::create_simulator(&config, &site_id)
                .ok_or_else(|| format!("Invalid site_id: {}", site_id))?;
            Ok(simulator.compute_wave_sound_field(&params, grid_res))
        }).await;

        histogram!("acoustic_compute_duration_seconds", "task" => "wave_field")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "wave_field").increment(1);

        result
    }

    pub async fn simulate_noise(&self, params: NoiseSimulationRequest) -> NoiseSimulationResult {
        let config = self.config.clone();
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let sim = NoiseSimulator::new(config);
            Ok(sim.simulate(&params))
        }).await.unwrap_or_else(|e| {
            counter!("acoustic_compute_errors_total", "task" => "noise_sim").increment(1);
            panic!("Noise simulation failed: {}", e);
        });

        histogram!("acoustic_compute_duration_seconds", "task" => "noise_sim")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "noise_sim").increment(1);

        result
    }

    pub async fn virtual_experience(
        &self,
        params: VirtualExperienceRequest,
        paths: Vec<SoundPath>,
        ir: Vec<f64>,
        t60: f64,
    ) -> VirtualExperienceResult {
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let vr_sim = VrEchoWallSimulator::new();
            Ok(vr_sim.simulate_experience(&params, paths, ir, t60))
        }).await.unwrap();

        histogram!("acoustic_compute_duration_seconds", "task" => "virtual_exp")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "virtual_exp").increment(1);

        result
    }

    pub async fn compute_binaural_ir(
        &self,
        src_pos: Vec3,
        listener_pos: Vec3,
        mono_ir: Vec<f64>,
        sample_rate: u32,
    ) -> crate::models::BinauralImpulseResponse {
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let vr_sim = VrEchoWallSimulator::new();
            Ok(vr_sim.compute_binaural_ir(&src_pos, &listener_pos, &mono_ir, sample_rate))
        }).await.unwrap();

        histogram!("acoustic_compute_duration_seconds", "task" => "binaural_ir")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "binaural_ir").increment(1);

        result
    }

    pub async fn compare_dynasties(&self, params: AcousticComparisonRequest) -> AcousticComparisonResult {
        let config = self.config.clone();
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let comparator = DesignComparator::new(config);
            Ok(comparator.compare_dynasties(&params))
        }).await.unwrap();

        histogram!("acoustic_compute_duration_seconds", "task" => "compare_dynasties")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "compare_dynasties").increment(1);

        result
    }

    pub async fn compare_eras(&self, params: AcousticComparisonRequest) -> AcousticComparisonResult {
        let config = self.config.clone();
        let start = Instant::now();

        let result = self.spawn_compute(move || {
            let comparator = EraComparator::new(config);
            Ok(comparator.compare_across_eras(&params))
        }).await.unwrap();

        histogram!("acoustic_compute_duration_seconds", "task" => "compare_eras")
            .record(start.elapsed().as_secs_f64());
        counter!("acoustic_compute_total", "task" => "compare_eras").increment(1);

        result
    }

    pub fn num_threads(&self) -> usize {
        self.thread_pool.current_num_threads()
    }

    pub fn generate_impulse_response(
        &self,
        params: SimulationParams,
        duration_sec: f64,
        sample_rate: u32,
    ) -> Result<Vec<f64>, String> {
        let site_id = params.site_id.clone();
        let simulator = Self::create_simulator(&self.config, &site_id)
            .ok_or_else(|| format!("Invalid site_id: {}", site_id))?;
        Ok(simulator.generate_impulse_response(&params, duration_sec, sample_rate))
    }

    fn create_simulator(config: &Arc<AcousticConfig>, site_id: &str) -> Option<AcousticSimulator> {
        let site_config = config.sites.get(site_id)?;
        let site_params = SiteAcousticParams::from_site_config(site_id, site_config);
        Some(AcousticSimulator::new(
            site_params,
            config.simulation_defaults.speed_of_sound,
            config.simulation_defaults.air_absorption_coefficient,
            config.simulation_defaults.diffraction_threshold,
        ))
    }

    async fn spawn_compute<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce() -> Result<T, String> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.thread_pool.spawn(move || {
            let result = f();
            let _ = tx.send(result);
        });

        rx.await.map_err(|e| format!("Compute task cancelled: {}", e))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AcousticConfig;

    #[test]
    fn test_pool_new() {
        let config = Arc::new(AcousticConfig::default());
        let pool = AcousticComputePool::with_num_threads(config, 2);
        assert_eq!(pool.num_threads(), 2);
    }

    #[tokio::test]
    async fn test_pool_invalid_site_trace_rays() {
        let config = Arc::new(AcousticConfig::default());
        let pool = AcousticComputePool::with_num_threads(config, 2);
        let params = SimulationParams {
            site_id: "invalid".to_string(),
            source_position: Vec3 { x: 0.0, y: 1.5, z: 0.0 },
            frequency: 1000.0,
            max_reflections: 5,
            num_rays: 10,
            temperature: 20.0,
            humidity: 50.0,
        };
        let result = pool.trace_rays(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid site_id"));
    }

    #[test]
    fn test_create_simulator_invalid_site() {
        let config = Arc::new(AcousticConfig::default());
        let sim = AcousticComputePool::create_simulator(&config, "nonexistent");
        assert!(sim.is_none());
    }
}
