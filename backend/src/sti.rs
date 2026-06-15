use crate::models::{SpeechIntelligibility, StiAnalysisParams};
use std::f64::consts::PI;

const STI_OCTAVE_BANDS: [f64; 7] = [125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];
const RASTI_BANDS: [f64; 5] = [500.0, 1000.0, 2000.0, 4000.0, 8000.0];
const STI_MODULATION_FREQS: [f64; 14] = [0.63, 0.8, 1.0, 1.25, 1.6, 2.0, 2.5, 3.15, 4.0, 5.0, 6.3, 8.0, 10.0, 12.5];
const STI_BAND_WEIGHTS: [f64; 7] = [0.026, 0.061, 0.147, 0.203, 0.206, 0.202, 0.155];
const RASTI_BAND_WEIGHTS: [f64; 5] = [0.15, 0.20, 0.20, 0.20, 0.25];

pub struct StiCalculator;

impl StiCalculator {
    pub fn analyze(params: &StiAnalysisParams) -> SpeechIntelligibility {
        let ir = &params.impulse_response;
        let fs = params.sample_rate;

        let (d50, c50) = Self::compute_definition_clarity(ir, fs);
        let center_time = Self::compute_center_time(ir, fs);
        let crispness = Self::compute_crispness(ir, fs);

        let (sti_value, band_snr) = Self::compute_sti(ir, fs, params.background_noise_level, params.speech_level);
        let rasti_value = Self::compute_rasti(ir, fs, params.background_noise_level, params.speech_level);

        SpeechIntelligibility {
            timestamp: chrono::Utc::now(),
            analysis_id: uuid::Uuid::new_v4(),
            site_id: params.site_id.clone(),
            site_name: match params.site_id.as_str() {
                "huiyinbi" => "回音壁".to_string(),
                "sanyinshi" => "三音石".to_string(),
                "huanqiutan" => "圜丘坛".to_string(),
                _ => params.site_id.clone(),
            },
            sti_value,
            rasti_value,
            crispness,
            definition_d50: d50,
            clarity_c50: c50,
            center_time,
            frequency_bands: STI_OCTAVE_BANDS.to_vec(),
            band_snr,
            speech_content: None,
        }
    }

    fn compute_sti(
        ir: &[f64],
        fs: u32,
        noise_level_db: f64,
        speech_level_db: f64,
    ) -> (f64, Vec<f64>) {
        let mut band_snr = Vec::with_capacity(7);
        let mut mtis = Vec::with_capacity(7);

        for band_idx in 0..7 {
            let fc = STI_OCTAVE_BANDS[band_idx];
            let filtered = Self::octave_band_filter(ir, fs, fc);
            let mut mti_band = Vec::with_capacity(14);

            for &fm in &STI_MODULATION_FREQS {
                let mtf = Self::compute_mtf(&filtered, fs, fm);
                let mti = if mtf > 0.001 {
                    let snr_eff = (speech_level_db - noise_level_db).clamp(-15.0, 15.0);
                    Self::snr_to_mti(mtf, snr_eff)
                } else {
                    0.0
                };
                mti_band.push(mti);
            }

            let avg_mti: f64 = mti_band.iter().sum::<f64>() / mti_band.len() as f64;
            let snr_band = if avg_mti > 0.0 && avg_mti < 1.0 {
                10.0 * (avg_mti / (1.0 - avg_mti)).log10()
            } else {
                speech_level_db - noise_level_db
            };
            band_snr.push(snr_band);
            mtis.push(avg_mti);
        }

        let sti: f64 = mtis.iter().enumerate()
            .map(|(i, &m)| STI_BAND_WEIGHTS[i] * m)
            .sum();

        (sti.clamp(0.0, 1.0), band_snr)
    }

    fn compute_rasti(
        ir: &[f64],
        fs: u32,
        noise_level_db: f64,
        speech_level_db: f64,
    ) -> f64 {
        let rasti_mod_freqs = [0.5, 1.0, 2.0, 4.0, 8.0];
        let mut weighted_sum = 0.0;

        for (band_idx, &fc) in RASTI_BANDS.iter().enumerate() {
            let filtered = Self::octave_band_filter(ir, fs, fc);
            let mut mtf_sum = 0.0;
            for &fm in &rasti_mod_freqs {
                mtf_sum += Self::compute_mtf(&filtered, fs, fm);
            }
            let avg_mtf = mtf_sum / rasti_mod_freqs.len() as f64;
            let snr_eff = (speech_level_db - noise_level_db).clamp(-15.0, 15.0);
            let mti = Self::snr_to_mti(avg_mtf, snr_eff);
            weighted_sum += RASTI_BAND_WEIGHTS[band_idx] * mti;
        }

        weighted_sum.clamp(0.0, 1.0)
    }

    fn snr_to_mti(mtf: f64, snr_db: f64) -> f64 {
        let snr_linear = 10.0_f64.powf(snr_db / 10.0);
        let mti = mtf * snr_linear / (1.0 + snr_linear);
        mti.clamp(0.0, 1.0)
    }

    fn compute_mtf(ir_band: &[f64], fs: u32, fm: f64) -> f64 {
        let n = ir_band.len();
        if n == 0 { return 0.0; }

        let mut num_real = 0.0;
        let mut num_imag = 0.0;
        let mut den = 0.0;

        for (i, &h) in ir_band.iter().enumerate() {
            let t = i as f64 / fs as f64;
            let phase = 2.0 * PI * fm * t;
            let h_sq = h * h;
            num_real += h_sq * phase.cos();
            num_imag += h_sq * phase.sin();
            den += h_sq;
        }

        if den < 1e-10 { return 0.0; }
        ((num_real * num_real + num_imag * num_imag).sqrt() / den).clamp(0.0, 1.0)
    }

    fn octave_band_filter(signal: &[f64], fs: u32, center_freq: f64) -> Vec<f64> {
        let q = 1.0 / (2.0_f64.sqrt());
        let fl = center_freq / (2.0_f64.sqrt());
        let fh = center_freq * (2.0_f64.sqrt());
        let _ = q;

        let wl = 2.0 * PI * fl / fs as f64;
        let wh = 2.0 * PI * fh / fs as f64;

        let al = wl.sin();
        let bl = (wl.cos()).cos().acos().cos();
        let ah = wh.sin();
        let bh = (wh.cos()).cos().acos().cos();

        let a0l = 1.0 + al;
        let a1l = -2.0 * bl;
        let a2l = 1.0 - al;
        let b0l = (1.0 - bl) / 2.0;
        let b1l = 1.0 - bl;
        let b2l = (1.0 - bl) / 2.0;

        let a0h = 1.0 + ah;
        let a1h = -2.0 * bh;
        let a2h = 1.0 - ah;
        let b0h = (1.0 + bh) / 2.0;
        let b1h = -(1.0 + bh);
        let b2h = (1.0 + bh) / 2.0;

        let high_passed = Self::apply_biquad(signal, b0h, b1h, b2h, a0h, a1h, a2h);
        let band_passed = Self::apply_biquad(&high_passed, b0l, b1l, b2l, a0l, a1l, a2l);
        band_passed
    }

    fn apply_biquad(signal: &[f64], b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Vec<f64> {
        let n = signal.len();
        let mut output = vec![0.0f64; n];
        let mut x1 = 0.0;
        let mut x2 = 0.0;
        let mut y1 = 0.0;
        let mut y2 = 0.0;

        for i in 0..n {
            let x0 = signal[i];
            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
            output[i] = y0;
            x2 = x1; x1 = x0;
            y2 = y1; y1 = y0;
        }
        output
    }

    pub fn compute_definition_clarity(ir: &[f64], fs: u32) -> (f64, f64) {
        let sample_50ms = (0.05 * fs as f64) as usize;
        let sample_500ms = (0.5 * fs as f64) as usize;
        let mut e_early50 = 0.0;
        let mut e_early500 = 0.0;
        let mut e_late = 0.0;

        for (i, &h) in ir.iter().enumerate() {
            let e = h * h;
            if i < sample_50ms { e_early50 += e; }
            if i < sample_500ms { e_early500 += e; } else { e_late += e; }
        }

        let total = e_early500 + e_late;
        let d50 = if total > 1e-10 { 100.0 * e_early50 / total } else { 0.0 };
        let c50 = if e_late > 1e-10 { 10.0 * (e_early500 / e_late).log10() } else { 10.0 };
        (d50.clamp(0.0, 100.0), c50.clamp(-10.0, 30.0))
    }

    pub fn compute_center_time(ir: &[f64], fs: u32) -> f64 {
        let mut num = 0.0;
        let mut den = 0.0;
        for (i, &h) in ir.iter().enumerate() {
            let e = h * h;
            let t = i as f64 / fs as f64;
            num += t * e;
            den += e;
        }
        if den > 1e-10 { num / den } else { 0.0 }
    }

    pub fn compute_crispness(ir: &[f64], fs: u32) -> f64 {
        let sample_20ms = (0.02 * fs as f64) as usize;
        let sample_100ms = (0.1 * fs as f64) as usize;
        let mut e_early = 0.0;
        let mut e_late = 0.0;

        for (i, &h) in ir.iter().enumerate() {
            let e = h * h;
            if i < sample_20ms { e_early += e; }
            else if i < sample_100ms { e_late += e; }
        }

        if e_late > 1e-10 { 10.0 * (e_early / e_late).log10() } else { 10.0 }
    }

    pub fn sti_quality_label(sti: f64) -> &'static str {
        match sti {
            v if v >= 0.85 => "优秀",
            v if v >= 0.75 => "良好",
            v if v >= 0.60 => "中等",
            v if v >= 0.45 => "较差",
            v if v >= 0.30 => "差",
            _ => "不可接受",
        }
    }
}
