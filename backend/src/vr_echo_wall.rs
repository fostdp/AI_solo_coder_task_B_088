use crate::models::{
    BinauralImpulseResponse, Vec3, VirtualExperienceRequest,
    VirtualExperienceResult, SoundPath, SpeechIntelligibility,
};
use std::f64::consts::PI;

pub struct VrEchoWallSimulator {
    playback_mode: String,
    headphone_optimized: bool,
}

impl Default for VrEchoWallSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl VrEchoWallSimulator {
    pub fn new() -> Self {
        Self {
            playback_mode: "headphone".to_string(),
            headphone_optimized: true,
        }
    }

    pub fn with_playback_mode(mut self, mode: &str) -> Self {
        self.playback_mode = mode.to_string();
        self
    }

    pub fn compute_binaural_ir(
        &self,
        src_pos: &Vec3,
        listener_pos: &Vec3,
        mono_ir: &[f64],
        sample_rate: u32,
    ) -> BinauralImpulseResponse {
        let head_radius = 0.0875;
        let src = src_pos.to_point3();
        let listener = listener_pos.to_point3();
        let sound_direction = (src - listener).normalize();
        let azimuth = sound_direction.x.atan2(sound_direction.z);
        let _elevation = sound_direction.y.asin();

        let (itd, ild_low, ild_mid, ild_high) = Self::compute_woodworth_itd_ild(azimuth, head_radius);
        let ild = ild_mid;

        let pinna_gain_db = Self::compute_pinna_gain(azimuth);
        let shoulder_reflection_delay = 0.000_300;
        let shoulder_reflection_gain = 0.15;

        let delay_samples = (itd.abs() * sample_rate as f64).round() as usize;
        let shoulder_delay_samples = (shoulder_reflection_delay * sample_rate as f64).round() as usize;

        let contralateral_atten = 10.0_f64.powf(-ild / 20.0);
        let pinna_linear = 10.0_f64.powf(pinna_gain_db / 20.0);

        let (left_ir, right_ir) = Self::apply_binaural_delays(
            mono_ir,
            delay_samples,
            shoulder_delay_samples,
            shoulder_reflection_gain,
            contralateral_atten,
            pinna_linear,
            itd >= 0.0,
        );

        BinauralImpulseResponse {
            left_ear: left_ir,
            right_ear: right_ir,
            sample_rate,
            listener_position: listener_pos.clone(),
            source_position: src_pos.clone(),
            itd_seconds: itd,
            ild_db: ild,
            azimuth_rad: azimuth,
            playback_mode: self.playback_mode.clone(),
            headphone_optimized: self.headphone_optimized,
            hrtf_notes: format!(
                "Woodworth ITD: {:.1}μs | ILD(band): low={:.1}dB mid={:.1}dB high={:.1}dB | pinna: {:+.1}dB | shoulder_refl: {:.0}μs",
                itd * 1e6, ild_low, ild_mid, ild_high, pinna_gain_db, shoulder_reflection_delay * 1e6
            ),
        }
    }

    pub fn compute_woodworth_itd_ild(azimuth: f64, head_radius: f64) -> (f64, f64, f64, f64) {
        let abs_az = azimuth.abs().min(PI / 2.0);
        let itd = (head_radius / 343.0) * (abs_az.sin() + abs_az);
        let itd = itd.copysign(azimuth);

        let ild_low = 1.5 * azimuth.sin().abs();
        let ild_mid = 6.0 * azimuth.sin().abs();
        let ild_high = 12.0 * azimuth.sin().abs();

        (itd, ild_low, ild_mid, ild_high)
    }

    pub fn compute_pinna_gain(azimuth: f64) -> f64 {
        let pinna_gain_front = 2.0;
        let pinna_gain_back = -3.0;
        if azimuth.abs() < PI / 3.0 {
            pinna_gain_front * (1.0 - azimuth.abs() / (PI / 3.0))
        } else {
            pinna_gain_back * ((azimuth.abs() - PI / 3.0) / (2.0 * PI / 3.0))
        }
    }

    pub fn apply_binaural_delays(
        mono_ir: &[f64],
        itd_delay_samples: usize,
        shoulder_delay_samples: usize,
        shoulder_gain: f64,
        contralateral_atten: f64,
        pinna_linear: f64,
        itd_positive: bool,
    ) -> (Vec<f64>, Vec<f64>) {
        let mut left_ir = Vec::with_capacity(mono_ir.len());
        let mut right_ir = Vec::with_capacity(mono_ir.len());

        if itd_positive {
            for i in 0..mono_ir.len() {
                let left_val = if i < itd_delay_samples { 0.0 } else { mono_ir[i - itd_delay_samples] };
                let mut right_val = mono_ir[i] * contralateral_atten;

                if i >= shoulder_delay_samples {
                    right_val += mono_ir[i - shoulder_delay_samples] * shoulder_gain * contralateral_atten;
                }

                left_ir.push(left_val * pinna_linear);
                right_ir.push(right_val);
            }
        } else {
            for i in 0..mono_ir.len() {
                let mut left_val = mono_ir[i] * contralateral_atten;
                let right_val = if i < itd_delay_samples { 0.0 } else { mono_ir[i - itd_delay_samples] };

                if i >= shoulder_delay_samples {
                    left_val += mono_ir[i - shoulder_delay_samples] * shoulder_gain * contralateral_atten;
                }

                left_ir.push(left_val);
                right_ir.push(right_val * pinna_linear);
            }
        }

        (left_ir, right_ir)
    }

    pub fn simulate_experience(
        &self,
        params: &VirtualExperienceRequest,
        paths: Vec<SoundPath>,
        ir: Vec<f64>,
        t60: f64,
    ) -> VirtualExperienceResult {
        let src = params.source_position.to_point3();
        let listener = params.listener_position.to_point3();
        let direct_dist = (listener - src).norm();

        let direct_path = SoundPath {
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
            angle_of_incidence: None,
        };

        let reflection_paths: Vec<SoundPath> = paths.iter()
            .filter(|p| p.reflection_count >= 1)
            .cloned()
            .collect();

        let total_paths = paths.clone();

        let binaural_ir = self.compute_binaural_ir(
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

        let noise_reduction = if params.include_noise { 0.15 } else { 0.0 };
        let sti_without_noise = base_sti;
        let sti_with_noise = (base_sti - noise_reduction).max(0.0);

        let speech_intelligibility = SpeechIntelligibility {
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

        let sound_preservation_score = match params.site_id.as_str() {
            "huiyinbi" => 0.92,
            "sanyinshi" => 0.85,
            _ => 0.78,
        };

        VirtualExperienceResult {
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

    pub fn get_playback_mode(&self) -> &str {
        &self.playback_mode
    }

    pub fn is_headphone_optimized(&self) -> bool {
        self.headphone_optimized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vr_simulator_new() {
        let sim = VrEchoWallSimulator::new();
        assert_eq!(sim.get_playback_mode(), "headphone");
        assert!(sim.is_headphone_optimized());
    }

    #[test]
    fn test_with_playback_mode() {
        let sim = VrEchoWallSimulator::new().with_playback_mode("speaker");
        assert_eq!(sim.get_playback_mode(), "speaker");
    }

    #[test]
    fn test_compute_woodworth_itd_ild_front() {
        let (itd, ild_low, ild_mid, ild_high) = VrEchoWallSimulator::compute_woodworth_itd_ild(0.0, 0.0875);
        assert_eq!(itd, 0.0);
        assert_eq!(ild_low, 0.0);
        assert_eq!(ild_mid, 0.0);
        assert_eq!(ild_high, 0.0);
    }

    #[test]
    fn test_compute_woodworth_itd_ild_side() {
        let (itd, _, ild_mid, _) = VrEchoWallSimulator::compute_woodworth_itd_ild(PI / 2.0, 0.0875);
        assert!(itd > 0.0006 && itd < 0.0007);
        assert!(ild_mid > 5.0 && ild_mid < 7.0);
    }

    #[test]
    fn test_compute_pinna_gain() {
        let gain_front = VrEchoWallSimulator::compute_pinna_gain(0.0);
        assert_eq!(gain_front, 2.0);

        let gain_back = VrEchoWallSimulator::compute_pinna_gain(PI);
        assert!(gain_back < 0.0);
    }

    #[test]
    fn test_apply_binaural_delays_positive_itd() {
        let mono_ir = vec![1.0, 0.5, 0.25, 0.125];
        let (left, right) = VrEchoWallSimulator::apply_binaural_delays(
            &mono_ir, 1, 1, 0.15, 0.5, 1.122, true
        );
        assert_eq!(left.len(), 4);
        assert_eq!(right.len(), 4);
        assert_eq!(left[0], 0.0);
        assert_eq!(left[1], 1.0 * 1.122);
        assert!(right[0] > 0.0);
    }

    #[test]
    fn test_apply_binaural_delays_negative_itd() {
        let mono_ir = vec![1.0, 0.5, 0.25, 0.125];
        let (left, right) = VrEchoWallSimulator::apply_binaural_delays(
            &mono_ir, 1, 1, 0.15, 0.5, 1.122, false
        );
        assert_eq!(right[0], 0.0);
        assert_eq!(right[1], 1.0 * 1.122);
        assert!(left[0] > 0.0);
    }

    #[test]
    fn test_compute_binaural_ir_front() {
        let sim = VrEchoWallSimulator::new();
        let src = Vec3 { x: 0.0, y: 1.5, z: -5.0 };
        let listener = Vec3 { x: 0.0, y: 1.5, z: 0.0 };
        let mono_ir = vec![1.0, 0.8, 0.5, 0.2];
        let binaural = sim.compute_binaural_ir(&src, &listener, &mono_ir, 44100);
        assert_eq!(binaural.itd_seconds, 0.0);
        assert_eq!(binaural.ild_db, 0.0);
        assert_eq!(binaural.playback_mode, "headphone");
        assert!(binaural.headphone_optimized);
        assert!(binaural.hrtf_notes.contains("Woodworth"));
    }

    #[test]
    fn test_compute_binaural_ir_left_side() {
        let sim = VrEchoWallSimulator::new();
        let src = Vec3 { x: -5.0, y: 1.5, z: 0.0 };
        let listener = Vec3 { x: 0.0, y: 1.5, z: 0.0 };
        let mono_ir = vec![1.0, 0.8, 0.5, 0.2];
        let binaural = sim.compute_binaural_ir(&src, &listener, &mono_ir, 44100);
        assert!(binaural.itd_seconds < 0.0);
        assert!(binaural.ild_db > 0.0);
        assert_eq!(binaural.azimuth_rad, -PI / 2.0);
    }

    #[test]
    fn test_itd_symmetry() {
        let (itd_pos, _, _, _) = VrEchoWallSimulator::compute_woodworth_itd_ild(0.5, 0.0875);
        let (itd_neg, _, _, _) = VrEchoWallSimulator::compute_woodworth_itd_ild(-0.5, 0.0875);
        assert!((itd_pos + itd_neg).abs() < 1e-10);
    }

    #[test]
    fn test_ild_monotonic_with_frequency() {
        let (_, ild_low, ild_mid, ild_high) = VrEchoWallSimulator::compute_woodworth_itd_ild(PI / 4.0, 0.0875);
        assert!(ild_low < ild_mid);
        assert!(ild_mid < ild_high);
    }

    #[test]
    fn test_shoulder_delay_samples() {
        let delay = 300e-6;
        let fs = 48000;
        let samples = (delay * fs as f64).round() as usize;
        assert_eq!(samples, 14);
    }
}
