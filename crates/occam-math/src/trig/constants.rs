//! Source: original
//! The owned trigonometry's constants, fitted and computed by `tools/coefficients/fit.py`. Generated: do not edit; run
//! `python3 -B tools/coefficients/fit.py` to regenerate it.

/// π/2 truncated to 33 significant bits, 1.5707963267341256142: the first of Cody and Waite's four pieces of π/2.
/// The first three have 33 bits each, so k times one is exact for k below 2^20.
pub(crate) const PIO2_C1: f64 = f64::from_bits(0x3FF9_21FB_5440_0000);

/// The next 33 significant bits of π/2, 6.0771005063039659766e-11: the second piece.
pub(crate) const PIO2_C2: f64 = f64::from_bits(0x3DD0_B461_1A60_0000);

/// The next 33 significant bits of π/2, 2.0222662487111664558e-21: the third piece.
pub(crate) const PIO2_C3: f64 = f64::from_bits(0x3BA3_198A_2E00_0000);

/// The rest of π/2 rounded to the nearest double, 8.4784276603688995700e-32: the four pieces sum to within 2^-159.8
/// of π/2.
pub(crate) const PIO2_C4: f64 = f64::from_bits(0x397B_839A_2520_49C1);

/// 2/π rounded to the nearest double, 0.63661977236758138243.
pub(crate) const INV_PIO2: f64 = f64::from_bits(0x3FE4_5F30_6DC9_C883);

/// π/2 rounded to the nearest double, 1.5707963267948965580.
pub(crate) const PIO2_HI: f64 = f64::from_bits(0x3FF9_21FB_5444_2D18);

/// π/2 less `PIO2_HI`, rounded to the nearest double, 6.1232339957367660359e-17.
pub(crate) const PIO2_LO: f64 = f64::from_bits(0x3C91_A626_3314_5C07);

/// π/4 rounded to the nearest double, 0.78539816339744827900.
pub(crate) const PIO4_HI: f64 = f64::from_bits(0x3FE9_21FB_5444_2D18);

/// π/4 less `PIO4_HI`, rounded to the nearest double, 3.0616169978683830179e-17.
pub(crate) const PIO4_LO: f64 = f64::from_bits(0x3C81_A626_3314_5C07);

/// π rounded to the nearest double, 3.1415926535897931160.
pub(crate) const PI_HI: f64 = f64::from_bits(0x4009_21FB_5444_2D18);

/// π less `PI_HI`, rounded to the nearest double, 1.2246467991473532072e-16.
pub(crate) const PI_LO: f64 = f64::from_bits(0x3CA1_A626_3314_5C07);

/// 3π/4 rounded to the nearest double, 2.3561944901923448370.
pub(crate) const THREE_PIO4_HI: f64 = f64::from_bits(0x4002_D97C_7F33_21D2);

/// 3π/4 less `THREE_PIO4_HI`, rounded to the nearest double, 9.1848509936051484375e-17.
pub(crate) const THREE_PIO4_LO: f64 = f64::from_bits(0x3C9A_7939_4C9E_8A0A);

/// atan(1/2) rounded to the nearest double, 0.46364760900080609352.
pub(crate) const ATAN_HALF_HI: f64 = f64::from_bits(0x3FDD_AC67_0561_BB4F);

/// atan(1/2) less `ATAN_HALF_HI`, rounded to the nearest double, 2.2698777452961687092e-17.
pub(crate) const ATAN_HALF_LO: f64 = f64::from_bits(0x3C7A_2B7F_222F_65E2);

/// The first 1280 bits of 2/π after the binary point, for Payne and Hanek's reduction. Word 0 holds bits 1 to 64,
/// bit 1 in its most significant place.
pub(crate) const TWO_OVER_PI: [u64; 20] = [
    0xA2F9_836E_4E44_1529,
    0xFC27_57D1_F534_DDC0,
    0xDB62_9599_3C43_9041,
    0xFE51_63AB_DEBB_C561,
    0xB724_6E3A_424D_D2E0,
    0x0649_2EEA_09D1_921C,
    0xFE1D_EB1C_B129_A73E,
    0xE882_35F5_2EBB_4484,
    0xE99C_7026_B45F_7E41,
    0x3991_D639_8353_39F4,
    0x9C84_5F8B_BDF9_283B,
    0x1FF8_97FF_DE05_980F,
    0xEF2F_118B_5A0A_6D1F,
    0x6D36_7ECF_27CB_09B7,
    0x4F46_3F66_9E5F_EA2D,
    0x7527_BAC7_EBE5_F17B,
    0x3D07_39F7_8A52_92EA,
    0x6BFB_5FB1_1F8D_5D08,
    0x5603_3046_FC7B_6BAB,
    0xF0CF_BC20_9AF4_361D,
];

/// Remez's minimax fit for sin(x) ≈ x + x·z·(S0 + S1·z + … + S5·z^5), z = x², lowest power first: within 2^-57.8
/// relative for |x| ≤ 0.786 as doubles, and 2^-57.9 before rounding.
pub(crate) const SIN: [f64; 6] = [
    f64::from_bits(0xBFC5_5555_5555_5548), // -0.16666666666666629659
    f64::from_bits(0x3F81_1111_1110_F730), // 0.0083333333333218406747
    f64::from_bits(0xBF2A_01A0_19BE_9217), // -1.9841269829358527846e-4
    f64::from_bits(0x3EC7_1DE3_5552_B52C), // 2.7557313538514233885e-6
    f64::from_bits(0xBE5A_E5E4_B83E_46F4), // -2.5050734389257542854e-8
    f64::from_bits(0x3DE5_D8B5_5949_5E1D), // 1.5895433289345186515e-10
];

/// Remez's minimax fit for cos(x) ≈ 1 − z/2 + z²·(C0 + C1·z + … + C5·z^5), z = x², lowest power first: within
/// 2^-63.9 relative for |x| ≤ 0.786 as doubles, and 2^-63.9 before rounding.
pub(crate) const COS: [f64; 6] = [
    f64::from_bits(0x3FA5_5555_5555_554B), // 0.041666666666666594965
    f64::from_bits(0xBF56_C16C_16C1_5009), // -0.0013888888888873315939
    f64::from_bits(0x3EFA_01A0_19C8_D25B), // 2.4801587288974347401e-5
    f64::from_bits(0xBE92_7E4F_7EFA_B97C), // -2.7557314206507112338e-7
    f64::from_bits(0x3E21_EE9D_A45A_F13C), // 2.0875703690362529572e-9
    f64::from_bits(0xBDA8_FA59_DB13_3B84), // -1.1358649130725861908e-11
];

/// Remez's minimax fit for atan(u) ≈ u + u·z·(A0 + A1·z + … + A8·z^8), z = u², lowest power first: within 2^-63.7
/// relative for |u| ≤ 0.2501 as doubles, and 2^-63.7 before rounding.
pub(crate) const ATAN: [f64; 9] = [
    f64::from_bits(0xBFD5_5555_5555_5553), // -0.33333333333333320381
    f64::from_bits(0x3FC9_9999_9999_8DAF), // 0.19999999999991532884
    f64::from_bits(0xBFC2_4924_923E_BBA4), // -0.14285714283820738491
    f64::from_bits(0x3FBC_71C7_138B_5C34), // 0.11111110903890236701
    f64::from_bits(0xBFB7_45CF_4D3D_7645), // -0.090908962590639921797
    f64::from_bits(0x3FB3_B0EA_64B4_35AF), // 0.076918267808841575550
    f64::from_bits(0xBFB1_09C1_459E_7B38), // -0.066555099007456486326
    f64::from_bits(0x3FAD_4F8B_F862_39B9), // 0.057247518606448229150
    f64::from_bits(0xBFA4_7A3E_0CAA_F9E2), // -0.039995135351535329726
];
