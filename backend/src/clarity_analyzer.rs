use crate::models::{
        AcousticComparisonRequest, AcousticComparisonResult,
        AnalyzerRequest, AlarmEvent, SpeechIntelligibility,
        StiAnalysisParams, StiWeightsConfig, AcousticConfig,
    };
use std::f64::consts::PI;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct AnalyzerTask {
    sti_config: StiWeightsConfig,
    config: Arc<AcousticConfig>,
    rx: mpsc::Receiver<AnalyzerRequest>,
    alarm_tx: mpsc::Sender<AlarmEvent>,
}

impl AnalyzerTask {
    pub fn new(
        sti_config: StiWeightsConfig,
        config: Arc<AcousticConfig>,
        rx: mpsc::Receiver<AnalyzerRequest>,
        alarm_tx: mpsc::Sender<AlarmEvent>,
    ) -> Self {
        Self { sti_config, config, rx, alarm_tx }
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
                AnalyzerRequest::CompareAcoustics { params, reply } => {
                    let result = Self::analyze_comparison(&self.sti_config, &self.config, &params);
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

    pub fn analyze_comparison(
        sti_config: &crate::models::StiWeightsConfig,
        config: &std::sync::Arc<crate::models::AcousticConfig>,
        params: &crate::models::AcousticComparisonRequest,
    ) -> crate::models::AcousticComparisonResult {
        use crate::models::*;
        use std::collections::HashMap;

        let mut site_metrics = Vec::new();

        for site_id in &params.site_ids {
            let metrics = Self::compute_site_metrics(site_id, config, params.frequency, params.background_noise_db);
            site_metrics.push(metrics);
        }

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

    fn compute_site_metrics(
        site_id: &str,
        config: &std::sync::Arc<crate::models::AcousticConfig>,
        frequency: f64,
        noise_db: f64,
    ) -> crate::models::SiteAcousticMetrics {
        use crate::models::*;

        if let Some(building) = config.ancient_buildings.get(site_id) {
            return Self::building_to_metrics(building, frequency, noise_db);
        }
        if let Some(hall) = config.concert_halls.get(site_id) {
            return Self::building_to_metrics(hall, frequency, noise_db);
        }

        let (base_t60, base_spl, description, name) = match site_id {
            "huiyinbi" => (2.2, 78.0, "天坛皇穹宇圆形围墙，直径61.5米，高3.72米", "回音壁"),
            "sanyinshi" => (1.2, 72.0, "皇穹宇殿前甬道上的三块石板", "三音石"),
            "huanqiutan" => (1.8, 75.0, "三层圆形石坛，上层直径23米，高5米", "圜丘坛"),
            _ => (2.0, 75.0, "未知场所", site_id),
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
            dynasty: if site_id == "huiyinbi" || site_id == "sanyinshi" || site_id == "huanqiutan" {
                Some("明/清".to_string())
            } else {
                None
            },
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

    fn building_to_metrics(
        building: &crate::models::BuildingMeta,
        frequency: f64,
        noise_db: f64,
    ) -> crate::models::SiteAcousticMetrics {
        use crate::models::*;

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
}
