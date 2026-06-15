use crate::models::{AnalyzerRequest, AlarmEvent, SpeechIntelligibility, StiAnalysisParams, StiWeightsConfig};
use std::f64::consts::PI;
use tokio::sync::mpsc;

pub struct AnalyzerTask {
    sti_config: StiWeightsConfig,
    rx: mpsc::Receiver<AnalyzerRequest>,
    alarm_tx: mpsc::Sender<AlarmEvent>,
}

impl AnalyzerTask {
    pub fn new(sti_config: StiWeightsConfig, rx: mpsc::Receiver<AnalyzerRequest>, alarm_tx: mpsc::Sender<AlarmEvent>) -> Self {
        Self { sti_config, rx, alarm_tx }
    }

    pub async fn run(&mut self) {
        while let Some(req) = self.rx.recv().await {
            match req {
                AnalyzerRequest::AnalyzeSti { params, reply } => {
                    let result = self.analyze_with_config(&params);
                    let _ = self.alarm_tx.send(AlarmEvent::CheckIntelligibility {
                        site_id: result.site_id.clone(),
                        sti: result.sti_value,
                        d50: result.definition_d50,
                        c50: result.clarity_c50,
                    }).await;
                    let _ = reply.send(result);
                }
            }
        }
    }

    fn analyze_with_config(&self, params: &StiAnalysisParams) -> SpeechIntelligibility {
        let ir = &params.impulse_response;
        let fs = params.sample_rate;
        let is_ancient = self.sti_config.default_mode == "ancient_chinese";

        let (d50, c50) = Self::compute_definition_clarity(ir, fs);
        let center_time = Self::compute_center_time(ir, fs);
        let crispness = Self::compute_crispness(ir, fs);

        let (sti_value, band_snr) = self.compute_sti(ir, fs, params.background_noise_level, params.speech_level, is_ancient);
        let rasti_value = self.compute_rasti(ir, fs, params.background_noise_level, params.speech_level, is_ancient);

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
            frequency_bands: self.sti_config.octave_bands.clone(),
            band_snr,
            speech_content: None,
        }
    }

    fn compute_sti(&self, ir: &[f64], fs: u32, noise_level_db: f64, speech_level_db: f64, is_ancient: bool) -> (f64, Vec<f64>) {
        let weights = if is_ancient {
            &self.sti_config.ancient_chinese_weights.sti
        } else {
            &self.sti_config.standard_weights.sti
        };
        let octave_bands = &self.sti_config.octave_bands;
        let mod_freqs = &self.sti_config.modulation_frequencies;
        let num_bands = octave_bands.len().min(weights.len());

        let mut band_snr = Vec::with_capacity(num_bands);
        let mut mtis = Vec::with_capacity(num_bands);

        for band_idx in 0..num_bands {
            let fc = octave_bands[band_idx];
            let filtered = Self::octave_band_filter(ir, fs, fc);
            let mut mti_band = Vec::with_capacity(mod_freqs.len());

            for &fm in mod_freqs {
                let mtf = Self::compute_mtf(&filtered, fs, fm);
                let tone_boost = if is_ancient && self.sti_config.ancient_chinese_weights.tone_affected_bands.contains(&band_idx) {
                    if self.sti_config.ancient_chinese_weights.tone_critical_mod_freqs.contains(&fm) {
                        self.sti_config.ancient_chinese_weights.tone_boost_factor
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                let mti = if mtf > 0.001 {
                    let snr_eff = (speech_level_db - noise_level_db).clamp(self.sti_config.snr_clamp_range[0], self.sti_config.snr_clamp_range[1]);
                    let raw_mti = Self::snr_to_mti(mtf, snr_eff);
                    (raw_mti * tone_boost).min(1.0)
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
            .map(|(i, &m)| weights[i] * m)
            .sum();

        (sti.clamp(0.0, 1.0), band_snr)
    }

    fn compute_rasti(&self, ir: &[f64], fs: u32, noise_level_db: f64, speech_level_db: f64, is_ancient: bool) -> f64 {
        let rasti_weights = if is_ancient {
            &self.sti_config.ancient_chinese_weights.rasti
        } else {
            &self.sti_config.standard_weights.rasti
        };
        let rasti_bands = if is_ancient {
            &self.sti_config.ancient_chinese_weights.rasti_bands
        } else {
            &self.sti_config.standard_weights.rasti_bands
        };
        let rasti_mod_freqs = if is_ancient {
            &self.sti_config.ancient_chinese_weights.rasti_mod_freqs
        } else {
            &self.sti_config.standard_weights.rasti_mod_freqs
        };

        let num_bands = rasti_bands.len().min(rasti_weights.len());
        let mut weighted_sum = 0.0;

        for band_idx in 0..num_bands {
            let fc = rasti_bands[band_idx];
            let filtered = Self::octave_band_filter(ir, fs, fc);
            let mut mtf_sum = 0.0;
            for &fm in rasti_mod_freqs {
                mtf_sum += Self::compute_mtf(&filtered, fs, fm);
            }
            let avg_mtf = mtf_sum / rasti_mod_freqs.len() as f64;
            let snr_eff = (speech_level_db - noise_level_db).clamp(self.sti_config.snr_clamp_range[0], self.sti_config.snr_clamp_range[1]);
            let mti = Self::snr_to_mti(avg_mtf, snr_eff);
            weighted_sum += rasti_weights[band_idx] * mti;
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
        let _q = 1.0 / (2.0_f64.sqrt());
        let fl = center_freq / (2.0_f64.sqrt());
        let fh = center_freq * (2.0_f64.sqrt());

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
