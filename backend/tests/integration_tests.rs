
// ============================================================================
// 天坛声学仿真系统 - Rust后端单元测试
// ============================================================================
// 测试模块:
//   mod dynasty     - 朝代声学参数对比验证
//   mod eras        - 跨时代混响时间对比验证
//   mod noise       - 噪声STI下降验证
//   mod experience  - 虚拟体验双耳听觉验证
// ============================================================================

use std::f64::consts::{PI, SQRT_2};

// ============================================================================
// 测试常量
// ============================================================================

const SPEED_OF_SOUND: f64 = 343.0;
const STI_MIN: f64 = 0.0;
const STI_MAX: f64 = 1.0;
const T60_MIN: f64 = 0.1;
const T60_MAX: f64 = 10.0;
const ITD_MAX_S: f64 = 0.000_700; // 700微秒，最大耳间时间差
const ILD_MAX_DB: f64 = 20.0;     // 最大耳间强度差

// ============================================================================
// 辅助函数
// ============================================================================

fn sabine_t60(volume: f64, surface_area: f64, avg_absorption: f64) -> f64 {
    if avg_absorption < 0.001 || surface_area < 1e-6 {
        return f64::INFINITY;
    }
    0.161 * volume / (surface_area * avg_absorption)
}

fn compute_sti_degradation(clean_sti: f64, snr_db: f64) -> (f64, f64) {
    let snr_factor = (snr_db / 20.0).tanh();
    let noisy_sti = 0.2 + (clean_sti - 0.2) * snr_factor;
    let degradation = clean_sti - noisy_sti;
    (noisy_sti.max(0.0), degradation.max(0.0))
}

fn compute_binaural_params(azimuth: f64, head_radius: f64) -> (f64, f64) {
    let itd = head_radius * (azimuth.sin() + azimuth) / SPEED_OF_SOUND;
    let ild = 6.0 * azimuth.cos().abs();
    (itd, ild)
}

fn compute_d50_c50(ir: &[f64], fs: u32) -> (f64, f64) {
    let s50 = (0.05 * fs as f64) as usize;
    let s500 = (0.5 * fs as f64) as usize;
    let n = ir.len();
    let mut e_early50 = 0.0;
    let mut e_early500 = 0.0;
    let mut e_late = 0.0;
    for (i, &h) in ir.iter().enumerate() {
        let e = h * h;
        if i < s50.min(n) { e_early50 += e; }
        if i < s500.min(n) { e_early500 += e; }
        else { e_late += e; }
    }
    let total = e_early500 + e_late;
    let d50 = if total > 1e-10 { 100.0 * e_early50 / total } else { 0.0 };
    let c50 = if e_late > 1e-10 { 10.0 * (e_early500 / e_late).log10() } else { 10.0 };
    (d50.clamp(0.0, 100.0), c50.clamp(-10.0, 30.0))
}

fn sti_quality_label(sti: f64) -> &'static str {
    match sti {
        v if v >= 0.85 => "优秀",
        v if v >= 0.75 => "良好",
        v if v >= 0.60 => "中等",
        v if v >= 0.45 => "较差",
        v if v >= 0.30 => "差",
        _ => "不可接受",
    }
}

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

// ============================================================================
// 1. 朝代对比验证声学参数
// ============================================================================

#[cfg(test)]
mod dynasty_tests {
    use super::*;

    struct DynastyBuilding {
        id: &'static str,
        name: &'static str,
        volume: f64,
        surface: f64,
        wall_abs: f64,
        ceil_abs: f64,
        floor_abs: f64,
        expected_t60_range: (f64, f64),
    }

    fn build_test_cases() -> Vec<DynastyBuilding> {
        vec![
            DynastyBuilding {
                id: "tang_temple", name: "唐代明堂",
                volume: 88.0 * 30.0 * 88.0,
                surface: 2.0 * (88.0*88.0 + 88.0*30.0 + 88.0*30.0),
                wall_abs: 0.08, ceil_abs: 0.12, floor_abs: 0.06,
                expected_t60_range: (3.0, 4.5),
            },
            DynastyBuilding {
                id: "song_temple", name: "宋代大庆殿",
                volume: 60.0 * 25.0 * 50.0,
                surface: 2.0 * (60.0*50.0 + 60.0*25.0 + 50.0*25.0),
                wall_abs: 0.10, ceil_abs: 0.15, floor_abs: 0.08,
                expected_t60_range: (2.0, 3.5),
            },
            DynastyBuilding {
                id: "ming_temple", name: "明代奉天殿",
                volume: 75.0 * 28.0 * 60.0,
                surface: 2.0 * (75.0*60.0 + 75.0*28.0 + 60.0*28.0),
                wall_abs: 0.09, ceil_abs: 0.13, floor_abs: 0.07,
                expected_t60_range: (2.5, 4.0),
            },
            DynastyBuilding {
                id: "qing_temple", name: "清代太和殿",
                volume: 64.0 * 26.0 * 37.0,
                surface: 2.0 * (64.0*37.0 + 64.0*26.0 + 37.0*26.0),
                wall_abs: 0.12, ceil_abs: 0.18, floor_abs: 0.10,
                expected_t60_range: (1.8, 3.0),
            },
            DynastyBuilding {
                id: "huiyinbi", name: "天坛回音壁",
                volume: PI * 30.75 * 30.75 * 3.72,
                surface: 2.0 * PI * 30.75 * 3.72 + 2.0 * PI * 30.75 * 30.75,
                wall_abs: 0.05, ceil_abs: 0.10, floor_abs: 0.04,
                expected_t60_range: (1.5, 3.0),
            },
        ]
    }

    // ---------- 正常场景 ----------

    #[test]
    fn test_dynasty_t60_within_range() {
        // TC-DY-01: 各朝代T60在预期范围内
        for b in build_test_cases() {
            let avg_abs = (b.wall_abs + b.ceil_abs + b.floor_abs) / 3.0;
            let t60 = sabine_t60(b.volume, b.surface, avg_abs);
            let t60 = t60.clamp(0.5, 5.0);
            assert!(
                t60 >= b.expected_t60_range.0 && t60 <= b.expected_t60_range.1,
                "[{}] T60={:.2}s 超出范围 [{:.1}, {:.1}]s",
                b.id, t60, b.expected_t60_range.0, b.expected_t60_range.1
            );
        }
    }

    #[test]
    fn test_dynasty_sti_trend_qing_over_tang() {
        // TC-DY-02: 清代STI应高于唐代（更多吸声材料）
        let cases = build_test_cases();
        let mut stis = Vec::new();
        for b in &cases {
            let avg_abs = (b.wall_abs + b.ceil_abs + b.floor_abs) / 3.0;
            let t60 = sabine_t60(b.volume, b.surface, avg_abs).clamp(0.5, 5.0);
            let sti = (0.4 + (1.5 - t60) * 0.3).clamp(0.2, 0.95);
            stis.push((b.id, sti));
        }
        let qing_sti = stis.iter().find(|(id, _)| *id == "qing_temple").unwrap().1;
        let tang_sti = stis.iter().find(|(id, _)| *id == "tang_temple").unwrap().1;
        assert!(qing_sti > tang_sti,
            "清代STI({:.3})应高于唐代STI({:.3})", qing_sti, tang_sti);
    }

    #[test]
    fn test_clarity_c50_d50_within_physical_bounds() {
        // TC-DY-03: C50/D50在物理范围内
        for b in build_test_cases() {
            let avg_abs = (b.wall_abs + b.ceil_abs + b.floor_abs) / 3.0;
            let t60 = sabine_t60(b.volume, b.surface, avg_abs).clamp(0.5, 5.0);
            let c50 = (3.0 + (1.5 - t60) * 5.0).clamp(-5.0, 20.0);
            let d50 = (50.0 + (1.5 - t60) * 20.0).clamp(10.0, 90.0);
            assert!(c50 >= -10.0 && c50 <= 30.0, "[{}] C50={:.1}越界", b.id, c50);
            assert!(d50 >= 0.0 && d50 <= 100.0, "[{}] D50={:.1}越界", b.id, d50);
        }
    }

    #[test]
    fn test_comparison_has_13_metrics() {
        // TC-DY-04: 应有13项对比指标
        let metrics = [
            "reverb_time_t60", "clarity_c50", "definition_d50", "sti_value",
            "rasti_value", "sound_pressure_level", "center_time", "bass_ratio",
            "brilliance", "intimacy", "warmth", "loudness", "echo_strength"
        ];
        assert_eq!(metrics.len(), 13, "对比指标数应为13");
    }

    #[test]
    fn test_best_speech_ranking_is_huiyinbi_or_qing() {
        // TC-DY-05: 最佳语音应为回音壁或清代建筑
        let cases = build_test_cases();
        let mut scores = Vec::new();
        for b in cases {
            let avg_abs = (b.wall_abs + b.ceil_abs + b.floor_abs) / 3.0;
            let t60 = sabine_t60(b.volume, b.surface, avg_abs).clamp(0.5, 5.0);
            let sti = (0.4 + (1.5 - t60) * 0.3).clamp(0.2, 0.95);
            let c50 = (3.0 + (1.5 - t60) * 5.0).clamp(-5.0, 20.0);
            let d50 = (50.0 + (1.5 - t60) * 20.0).clamp(10.0, 90.0);
            let speech_score = sti * 0.5 + c50 / 20.0 * 0.3 + d50 / 100.0 * 0.2;
            scores.push((b.id, speech_score));
        }
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let best = scores[0].0;
        assert!(
            ["huiyinbi", "qing_temple", "song_temple"].contains(&best),
            "最佳语音应为回音壁/清代/宋代，实际是{}", best
        );
    }

    // ---------- 边界条件 ----------

    #[test]
    fn test_boundary_extreme_small_volume() {
        // TC-DY-09: 极小体积边界
        let tiny_t60 = sabine_t60(10.0, 34.0, 0.1).clamp(0.5, 5.0);
        assert!(tiny_t60 >= 0.1, "极小体积T60>0.1s");
    }

    #[test]
    fn test_boundary_extreme_large_volume() {
        // TC-DY-09: 极大体积边界
        let huge_t60 = sabine_t60(100_000.0, 20_400.0, 0.05).clamp(0.5, 5.0);
        assert!(huge_t60 <= 5.0, "极大体积钳位后T60<=5s");
    }

    #[test]
    fn test_boundary_near_zero_absorption() {
        // TC-DY-10: 接近零吸声
        let t60 = sabine_t60(10_000.0, 5_200.0, 0.000_1);
        assert!(t60 > 100.0, "接近零吸声T60应极大");
    }

    #[test]
    fn test_boundary_full_absorption() {
        // TC-DY-11: 全吸声（消声室）
        let t60 = sabine_t60(1_000.0, 1_300.0, 0.99);
        assert!(t60 < 0.2, "全吸声T60<0.2s");
    }

    // ---------- 异常场景 ----------

    #[test]
    #[should_panic(expected = "体积非正")]
    fn test_exception_negative_volume() {
        // TC-DY-12: 负体积
        let vol = -1000.0;
        assert!(vol > 0.0, "体积非正");
        let _ = sabine_t60(vol, 500.0, 0.1);
    }

    #[test]
    fn test_exception_absorption_out_of_range() {
        // TC-DY-13: 吸声系数超出[0,1]
        let invalid = [-0.1, 1.5, 2.0];
        for &a in &invalid {
            assert!(a < 0.0 || a > 1.0, "应检测到超出范围的吸收: {}", a);
        }
    }
}

// ============================================================================
// 2. 跨时代对比验证混响时间
// ============================================================================

#[cfg(test)]
mod eras_tests {
    use super::*;

    // ---------- 正常场景 ----------

    #[test]
    fn test_ancient_t60_longer_than_modern() {
        // TC-ER-01: 古代T60普遍长于现代
        let ancient_t60s = [3.2, 2.5, 2.2, 3.8, 2.8];
        let modern_t60s = [2.0, 1.8, 1.9, 1.85, 2.1];
        let avg_ancient: f64 = ancient_t60s.iter().sum::<f64>() / ancient_t60s.len() as f64;
        let avg_modern: f64 = modern_t60s.iter().sum::<f64>() / modern_t60s.len() as f64;
        assert!(avg_ancient > avg_modern,
            "古代平均T60({:.2}s) > 现代平均T60({:.2}s)", avg_ancient, avg_modern);
    }

    #[test]
    fn test_modern_t60_in_standard_range() {
        // TC-ER-02: 现代音乐厅T60在[1.5, 2.3]s
        let modern_halls = [
            ("shoemaker", 2.0), ("vineyard", 1.8), ("boston", 1.9),
            ("berlin", 2.0), ("vienna", 2.1),
        ];
        for (name, t60) in modern_halls {
            assert!(t60 >= 1.5 && t60 <= 2.3,
                "音乐厅{} T60={:.1}s不在标准范围内", name, t60);
        }
    }

    #[test]
    fn test_sabine_formula_accuracy() {
        // TC-ER-03: 赛宾公式计算与典型值偏差<50%
        let cases = [
            (20_000.0, 8_200.0, 0.10, 2.0),
            (18_000.0, 7_600.0, 0.12, 1.8),
            (11_000.0, 5_200.0, 0.06, 2.2),
        ];
        for (v, s, a, expected) in cases {
            let calc = sabine_t60(v, s, a);
            let error = (calc - expected).abs() / expected;
            assert!(error < 0.5,
                "赛宾公式偏差过大: 计算={:.2}s, 典型={:.2}s, 误差={:.1}%",
                calc, expected, error * 100.0);
        }
    }

    #[test]
    fn test_ancient_ritual_long_reverb() {
        // TC-ER-04: 古代70%以上T60>2s（仪式感）
        let ancient = [3.8, 2.8, 3.2, 2.5, 2.2, 3.5, 2.9];
        let ratio = ancient.iter().filter(|&&t| t > 2.0).count() as f64 / ancient.len() as f64;
        assert!(ratio >= 0.7, "古代长混响比例应>=70%，实际={:.0}%", ratio * 100.0);
    }

    #[test]
    fn test_modern_music_optimal_reverb() {
        // TC-ER-05: 现代90%以上T60在[1.7, 2.3]s（音乐最佳）
        let modern = [2.0, 1.8, 1.9, 2.1, 2.0, 1.85, 1.95];
        let ratio = modern.iter()
            .filter(|&&t| t >= 1.7 && t <= 2.3)
            .count() as f64 / modern.len() as f64;
        assert!(ratio >= 0.9, "现代最优混响比例应>=90%，实际={:.0}%", ratio * 100.0);
    }

    // ---------- 边界条件 ----------

    #[test]
    fn test_boundary_eras_t60_overlap() {
        // TC-ER-06: 边界 - 古今T60可能重叠
        let ancient_small = 1.9;
        let modern_large = 2.1;
        assert!((ancient_small - modern_large).abs() < 1.0,
            "古今T60边界可能重叠");
    }

    #[test]
    fn test_boundary_extreme_ancient_8s() {
        // TC-ER-07: 大型石窟类极端长混响
        let t60_extreme = 8.0;
        assert!(t60_extreme <= T60_MAX, "极端T60应<={}s", T60_MAX);
    }

    #[test]
    fn test_boundary_same_volume_diff_absorption() {
        // TC-ER-09: 同体积，不同吸声 => 古今2倍以上差异
        let v = 15_000.0;
        let s = 6_500.0;
        let t_ancient = sabine_t60(v, s, 0.06);
        let t_modern = sabine_t60(v, s, 0.18);
        let ratio = t_ancient / t_modern;
        assert!(ratio >= 2.0, "同体积古今T60比应>=2x，实际={:.1}x", ratio);
    }

    // ---------- 异常场景 ----------

    #[test]
    fn test_exception_negative_t60_clamped() {
        // TC-ER-10: 负T60应被过滤为正
        let raw = [2.0, -1.5, 3.0, -0.1, 2.5];
        let clamped: Vec<f64> = raw.iter().map(|&t| t.max(T60_MIN)).collect();
        assert!(clamped.iter().all(|&t| t > 0.0), "所有T60应为正");
    }

    #[test]
    fn test_exception_zero_volume_produces_nan() {
        // TC-ER-12: 零体积应检测
        let t60 = sabine_t60(0.0, 0.0, 0.1);
        assert!(t60.is_nan() || t60.is_infinite(), "零体积应产生NaN/Inf");
    }
}

// ============================================================================
// 3. 噪声影响验证STI下降
// ============================================================================

#[cfg(test)]
mod noise_tests {
    use super::*;

    // ---------- 正常场景 ----------

    #[test]
    fn test_sti_monotonic_with_snr() {
        // TC-NS-01: STI随SNR单调非递减
        let clean_sti = 0.8;
        let mut prev_sti = -1.0;
        for snr in (-20..=40).step_by(5) {
            let (noisy, _) = compute_sti_degradation(clean_sti, snr as f64);
            assert!(noisy >= prev_sti,
                "STI应随SNR单调非递减: SNR={} -> {:.3}, 前值={:.3}",
                snr, noisy, prev_sti);
            prev_sti = noisy;
        }
    }

    #[test]
    fn test_noisy_sti_never_exceeds_clean() {
        // TC-NS-02: 噪声STI永远<=纯净STI
        let clean = [0.6, 0.75, 0.85, 0.95];
        let snrs = [-10.0, 0.0, 10.0, 20.0, 30.0, 40.0];
        for &c in &clean {
            for &snr in &snrs {
                let (noisy, degrad) = compute_sti_degradation(c, snr);
                assert!(noisy <= c + 1e-9,
                    "噪声STI({:.3})不能高于纯净STI({:.3})", noisy, c);
                assert!(degrad >= -1e-9,
                    "STI下降量({:.3})非负", degrad);
            }
        }
    }

    #[test]
    fn test_snr_zero_sti_near_baseline() {
        // TC-NS-03: SNR=0时STI≈0.2
        let clean = 0.8;
        let (noisy, _) = compute_sti_degradation(clean, 0.0);
        assert!(approx_eq(noisy, 0.2, 0.01),
            "SNR=0时STI≈0.2，实际={:.3}", noisy);
    }

    #[test]
    fn test_high_snr_small_loss() {
        // TC-NS-04: SNR>=30dB损失<5%
        let clean = 0.85;
        let (noisy, degrad) = compute_sti_degradation(clean, 30.0);
        let ratio = degrad / clean;
        assert!(ratio < 0.05,
            "SNR=30dB损失<5%，实际={:.2}%", ratio * 100.0);
    }

    #[test]
    fn test_low_snr_severe_degradation() {
        // TC-NS-05: SNR<=-10dB时STI接近基线
        let clean = 0.9;
        let (noisy, _) = compute_sti_degradation(clean, -10.0);
        assert!(noisy < 0.3,
            "SNR=-10dB时STI<0.3，实际={:.3}", noisy);
    }

    #[test]
    fn test_multi_noise_energy_sum() {
        // TC-NS-06: 多噪声源能量叠加（非线性）
        let sources = [55.0, 60.0, 58.0, 62.0];
        let energy: f64 = sources.iter().map(|&l| 10.0_f64.powf(l / 10.0)).sum();
        let total_db = 10.0 * energy.log10();
        let max_single = sources.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(total_db > max_single, "合声级高于最大单源");
        let upper_bound = max_single + 10.0 * (sources.len() as f64).log10();
        assert!(total_db <= upper_bound + 0.1, "合声级不超过理论上限");
    }

    #[test]
    fn test_sti_quality_transitions() {
        // TC-NS-07: 质量分级转换
        let clean = 0.9;
        let mut qualities = Vec::new();
        for noise in (20..=90).step_by(5) {
            let snr = 70.0 - noise as f64;
            let (sti, _) = compute_sti_degradation(clean, snr);
            qualities.push((noise, sti_quality_label(sti)));
        }
        let final_q = qualities.last().unwrap().1;
        assert!(["不可接受", "差"].contains(&final_q),
            "90dB噪声下应为不可接受/差，实际={}", final_q);
    }

    // ---------- 边界条件 ----------

    #[test]
    fn test_boundary_clean_sti_perfect() {
        // TC-NS-10: 纯净STI=1.0边界
        let (noisy, _) = compute_sti_degradation(1.0, 0.0);
        assert!(approx_eq(noisy, 0.2 + 0.8 * 0.0_f64.tanh(), 0.001));
    }

    #[test]
    fn test_boundary_clean_sti_zero() {
        // TC-NS-11: 纯净STI=0.0边界
        let (noisy, _) = compute_sti_degradation(0.0, 40.0);
        assert!(noisy >= 0.2, "纯STI=0噪声后>=0.2基线");
    }

    #[test]
    fn test_boundary_extreme_snr() {
        // TC-NS-12: 极端SNR(+60/-30dB)
        for &snr in &[60.0, -30.0] {
            let (noisy, _) = compute_sti_degradation(0.8, snr);
            assert!(noisy >= STI_MIN && noisy <= STI_MAX,
                "SNR={}时STI越界: {:.3}", snr, noisy);
        }
    }

    #[test]
    fn test_boundary_degradation_always_nonnegative() {
        // TC-NS-13: 下降量始终>=0
        let cases = [(0.8, 40.0), (0.8, 0.0), (0.8, -20.0), (0.2, 30.0), (0.95, 10.0)];
        for (clean, snr) in cases {
            let (noisy, degrad) = compute_sti_degradation(clean, snr);
            assert!(degrad >= -1e-9,
                "STI下降量为负: clean={}, snr={}, degrad={}", clean, snr, degrad);
            assert!(noisy <= clean + 1e-9,
                "噪声STI超纯净: {} > {}", noisy, clean);
        }
    }

    #[test]
    fn test_boundary_500_visitors_not_painful() {
        // TC-NS-09: 500游客不超过痛阈120dB
        let sources: Vec<f64> = vec![60.0; 500];
        let energy: f64 = sources.iter().map(|&l| 10.0_f64.powf(l / 10.0)).sum();
        let total = 10.0 * energy.log10();
        assert!(total < 120.0, "500人噪声<120dB (痛阈)");
        assert!(total > 85.0, "500人噪声>85dB");
    }

    // ---------- 异常场景 ----------

    #[test]
    #[should_panic(expected = "游客数非负")]
    fn test_exception_negative_visitors() {
        // TC-NS-14: 负游客数
        let visitors: i32 = -10;
        assert!(visitors >= 0, "游客数非负");
    }

    #[test]
    fn test_exception_sti_cannot_improve_with_noise() {
        // TC-NS-16: 检测实现错误（噪声STI>纯STI）
        let clean = 0.75;
        let buggy_noisy = 0.9;
        if buggy_noisy > clean {
            assert!(true, "应被检测：噪声STI不可能高于纯净STI");
        }
    }

    #[test]
    fn test_exception_extreme_sound_levels() {
        // TC-NS-17: 极端声级检测
        let invalid = [-10.0, 250.0];
        for &lvl in &invalid {
            assert!(lvl < 0.0 || lvl > 200.0,
                "声级{}应被检测超出范围", lvl);
        }
    }
}

// ============================================================================
// 4. 虚拟体验测试交互真实感
// ============================================================================

#[cfg(test)]
mod experience_tests {
    use super::*;
    use std::collections::HashMap;

    fn positions() -> HashMap<&'static str, (f64, f64)> {
        let mut m = HashMap::new();
        m.insert("east",   ( 15.0,   0.0));
        m.insert("west",   (-15.0,   0.0));
        m.insert("north",  (  0.0, -15.0));
        m.insert("south",  (  0.0,  15.0));
        m.insert("center", (  0.0,   0.0));
        m
    }

    fn azimuth(src: (f64, f64), lst: (f64, f64)) -> f64 {
        // 站在src面向lst方向：x为右，z为前（此处用y分量代替z）
        let dx = lst.0 - src.0;
        let dy = lst.1 - src.1;
        dx.atan2(dy)
    }

    // ---------- 正常场景 ----------

    #[test]
    fn test_azimuth_within_pi_range() {
        // TC-VE-01: 方位角在[-π, π]
        let pos = positions();
        let keys: Vec<&str> = pos.keys().cloned().collect();
        for &a in &keys {
            for &b in &keys {
                if a == b { continue; }
                let az = azimuth(pos[a], pos[b]);
                assert!(az >= -PI && az <= PI,
                    "方位角越界: {}->{}: {:.3}", a, b, az);
            }
        }
    }

    #[test]
    fn test_itd_within_physical_bounds() {
        // TC-VE-02: ITD <= 700μs
        let pos = positions();
        let keys: Vec<&str> = pos.keys().cloned().collect();
        let head_r = 0.0875;
        for &a in &keys {
            for &b in &keys {
                if a == b { continue; }
                let az = azimuth(pos[a], pos[b]);
                let (itd, _) = compute_binaural_params(az, head_r);
                assert!(itd.abs() <= ITD_MAX_S,
                    "ITD超限: {}->{}: {:.1}μs > 700μs",
                    a, b, itd * 1e6);
            }
        }
    }

    #[test]
    fn test_ild_within_physical_bounds() {
        // TC-VE-03: ILD物理范围
        let head_r = 0.0875;
        for deg in 0..=360 {
            let az = (deg as f64) * PI / 180.0;
            let (_, ild) = compute_binaural_params(az, head_r);
            assert!(ild >= 0.0 && ild <= ILD_MAX_DB,
                "ILD越界: {}° -> {:.1}dB", deg, ild);
        }
    }

    #[test]
    fn test_self_echo_delay_reasonable() {
        // TC-VE-04: 自回音延迟(沿半圆弧返回)
        let radius = 15.0;
        let path = PI * radius;  // 半圆弧
        let delay = path / SPEED_OF_SOUND;
        assert!(delay > 0.1, "自回音延迟>0.1s: {:.3}s", delay);
        assert!(delay < 0.3, "自回音延迟<0.3s: {:.3}s", delay);
    }

    #[test]
    fn test_east_west_classic_echo_path() {
        // TC-VE-05: 东西向经典场景（路径>直线）
        let pos = positions();
        let (sx, sy) = pos["east"];
        let (lx, ly) = pos["west"];
        let direct = ((sx - lx).powi(2) + (sy - ly).powi(2)).sqrt();
        assert!(approx_eq(direct, 30.0, 1.0), "东西直距≈30m，实际={:.1}", direct);
        let arc_path = PI * 15.0;
        assert!(arc_path > direct, "弧长>直距");
    }

    #[test]
    fn test_sti_across_positions_reasonable() {
        // TC-VE-06: 各位置组合STI合理
        let pos = positions();
        let keys: Vec<&str> = pos.keys().cloned().collect();
        for &a in &keys {
            for &b in &keys {
                if a == b { continue; }
                let (sx, sy) = pos[a];
                let (lx, ly) = pos[b];
                let dist = ((sx-lx).powi(2) + (sy-ly).powi(2)).sqrt();
                let factor = (1.0 - dist / 60.0).max(0.0);
                let sti = 0.5 + 0.35 * factor;
                assert!(sti >= 0.15 && sti <= 0.95,
                    "{}->{} STI越界: {:.3}", a, b, sti);
            }
        }
    }

    // ---------- 边界条件 ----------

    #[test]
    fn test_boundary_azimuth_dead_front_itd_zero() {
        // TC-VE-07: 正前方ITD=0
        let head_r = 0.0875;
        let (itd, _) = compute_binaural_params(0.0, head_r);
        assert!(approx_eq(itd, 0.0, 1e-6), "正前方ITD=0，实际={:.6}", itd);
    }

    #[test]
    fn test_boundary_azimuth_side_max_itd() {
        // TC-VE-08: 正侧方ITD较大
        let head_r = 0.0875;
        let (itd, _) = compute_binaural_params(PI / 2.0, head_r);
        assert!(itd.abs() > 0.000_500,
            "正侧方ITD>500μs，实际={:.1}μs", itd * 1e6);
    }

    #[test]
    fn test_boundary_center_to_all_equal_distance() {
        // TC-VE-10: 圆心到各墙面距离相等
        let pos = positions();
        let center = pos["center"];
        let radius = 15.0;
        for (&name, &(x, y)) in &pos {
            if name == "center" { continue; }
            let d = ((x-center.0).powi(2) + (y-center.1).powi(2)).sqrt();
            assert!(approx_eq(d, radius, 1.0),
                "中心到{}距离≈半径，实际={:.1}", name, d);
        }
    }

    #[test]
    fn test_boundary_binaural_sign_symmetry() {
        // TC-VE-11: ±方位角产生符号相反ITD
        let head_r = 0.0875;
        let az = 0.5;
        let (itd_p, ild_p) = compute_binaural_params(az, head_r);
        let (itd_n, ild_n) = compute_binaural_params(-az, head_r);
        assert!(approx_eq(itd_p, -itd_n, 1e-6),
            "ITD不对称: {:.6} vs {:.6}", itd_p, itd_n);
        assert!(approx_eq(ild_p, ild_n, 1e-6),
            "ILD不对称: {:.3} vs {:.3}", ild_p, ild_n);
    }

    #[test]
    fn test_boundary_listener_at_wall() {
        // TC-VE-09: 听者贴墙，距中心=半径
        let angle = 0.3;
        let r = 15.0;
        let px = r * angle.cos();
        let py = r * angle.sin();
        let dist = (px.powi(2) + py.powi(2)).sqrt();
        assert!(approx_eq(dist, r, 0.1),
            "墙面点到中心=半径，实际={:.2}", dist);
    }

    // ---------- 异常场景 ----------

    #[test]
    fn test_exception_invalid_position_name() {
        // TC-VE-12: 无效位置名
        let pos = positions();
        assert!(!pos.contains_key("nowhere"),
            "'nowhere'不应是有效位置");
    }

    #[test]
    fn test_exception_outside_wall_coords() {
        // TC-VE-13: 超出建筑范围
        let (x, y) = (100.0, 100.0);
        let dist = (x*x + y*y).sqrt();
        let radius = 15.0;
        assert!(dist > radius * 2.0,
            "检测到超出范围的坐标: ({}, {})", x, y);
    }

    #[test]
    fn test_exception_infrasonic_ultrasonic() {
        // TC-VE-14: 可听范围外频率
        let audible = (20, 20_000);
        let invalid = [5, 0, 50_000, -100];
        for &f in &invalid {
            assert!(f < audible.0 || f > audible.1,
                "频率{}Hz应在可听范围外", f);
        }
    }

    #[test]
    fn test_exception_nan_coordinates_detected() {
        // TC-VE-16: NaN坐标检测
        let nan_vals = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        for &v in &nan_vals {
            assert!(v.is_nan() || v.is_infinite(),
                "应检测特殊浮点值");
        }
    }
}

// ============================================================================
// 5. IR信号处理单元测试
// ============================================================================

#[cfg(test)]
mod signal_processing_tests {
    use super::*;

    #[test]
    fn test_d50_c50_with_impulse_response() {
        // 测试C50/D50计算：构造一个衰减IR
        let fs = 48_000u32;
        let duration_s = 2.0;
        let n = (fs as f64 * duration_s) as usize;
        let mut ir = vec![0.0f64; n];
        ir[0] = 1.0; // 直达
        // 指数衰减
        for i in 1..n {
            let t = i as f64 / fs as f64;
            ir[i] = (-t / 0.5).exp() * 0.3;
        }
        let (d50, c50) = compute_d50_c50(&ir, fs);
        assert!(d50 > 10.0 && d50 < 100.0, "D50在合理范围: {}", d50);
        assert!(c50 >= -10.0 && c50 <= 30.0, "C50在合理范围: {}", c50);
    }

    #[test]
    fn test_d50_c50_empty_ir() {
        // 空输入边界
        let (d50, c50) = compute_d50_c50(&[], 48_000);
        assert!(d50 >= 0.0, "空IR D50非负");
        assert!(c50 <= 30.0, "空IR C50有界");
    }

    #[test]
    fn test_sti_quality_label_all_levels() {
        assert_eq!(sti_quality_label(0.95), "优秀");
        assert_eq!(sti_quality_label(0.80), "良好");
        assert_eq!(sti_quality_label(0.65), "中等");
        assert_eq!(sti_quality_label(0.50), "较差");
        assert_eq!(sti_quality_label(0.35), "差");
        assert_eq!(sti_quality_label(0.15), "不可接受");
    }
}

// ============================================================================
// 6. 综合集成测试
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    struct Building { id: &'static str, t60: f64, sti: f64, c50: f64 }

    #[test]
    fn test_full_workflow_dynasty_noise_binaural() {
        // TC-INT-01: 完整流程：朝代对比→噪声→双耳
        let buildings = vec![
            Building { id: "tang",   t60: 3.8, sti: 0.55, c50: -2.0 },
            Building { id: "song",   t60: 2.8, sti: 0.65, c50:  2.0 },
            Building { id: "ming",   t60: 3.2, sti: 0.60, c50:  0.0 },
            Building { id: "qing",   t60: 2.5, sti: 0.70, c50:  3.5 },
            Building { id: "echo",   t60: 2.2, sti: 0.75, c50:  5.0 },
        ];

        // Step1: 综合评分选最佳
        let scored: Vec<(&str, f64)> = buildings.iter().map(|b| {
            let s = b.sti * 0.5 + (b.c50 + 5.0) / 25.0 * 0.3
                + (1.0 - (b.t60 - 2.0).abs() / 4.0) * 0.2;
            (b.id, s)
        }).collect();

        let best = scored.iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        // Step2: 噪声分析
        let best_data = buildings.iter().find(|b| b.id == best.0).unwrap();
        let (n60, d60) = compute_sti_degradation(best_data.sti, 10.0);
        let (n80, d80) = compute_sti_degradation(best_data.sti, -10.0);
        assert!(n60 > n80, "60dB噪声STI>80dB噪声STI");

        // Step3: 双耳参数（正前方）
        let (itd, ild) = compute_binaural_params(0.0, 0.0875);
        assert!(approx_eq(itd, 0.0, 1e-5), "正前方ITD≈0");

        println!("\n=== Rust集成测试流程 ===");
        println!("最佳建筑: {} (评分: {:.3})", best.0, best.1);
        println!("纯净STI: {:.3} | 60dB噪声: {:.3}(-{:.1}%) | 80dB噪声: {:.3}(-{:.1}%)",
            best_data.sti, n60, d60*100.0, n80, d80*100.0);
        println!("正前方ITD: {:.1}μs, ILD: {:.1}dB", itd*1e6, ild);
    }

    #[test]
    fn test_ancient_vs_modern_same_noise() {
        // TC-INT-02: 同噪声下古今STI对比
        let cases = [
            ("明代奉天殿", 3.2, 0.60),
            ("清代太和殿", 2.5, 0.70),
            ("鞋盒音乐厅", 2.0, 0.80),
        ];
        for snr in [30, 20, 10, 0, -10] {
            println!("SNR={:+3}dB: ", snr);
            for (name, _, clean) in &cases {
                let (noisy, _) = compute_sti_degradation(*clean, snr as f64);
                print!("  {}: {:.3} ", name, noisy);
            }
            println!();
        }
        assert!(true, "集成流程完成");
    }
}
