//! Source: arm-optimized-routines
//! [`exp`] and [`expm1`], which `occam-math` owns (`docs/decisions/2026-11-11-exp-from-arm.md`): Arm's `exp` from
//! optimized-routines v26.07 (`math/exp.c` and its table and coefficients from `math/exp_data.c`, without FMA) as
//! phasekit ported it, and phasekit's own `expm1` on its reduction. Arm's part is Copyright (c) 2018-2025, Arm Limited,
//! licensed MIT OR Apache-2.0 WITH LLVM-exception and taken here under MIT, as `REUSE.toml` declares.
//!
//! exp(x) = 2^(k/128) exp(r), with x = k ln 2/128 + r and |r| ≤ ln 2/256. The table gives 2^(k/128) as H (1 + T), so
//! exp(x) ≈ scale + scale tmp, with scale = H 2^⌊k/128⌋ and tmp = T + exp(r) − 1: within 0.511 ulp by Arm's bound. k is
//! rounded by adding 1.5 2^52, never by a conversion, and read back from the sum's low bits.
//!
//! Both read a table and branch on their argument, so they have no lane form: in lanes, each lane runs alone through
//! [`Real::map`] and gets the bits of a single call.

use crate::exact::two_sum;
use crate::real::Real;

/// 128/ln 2.
const INV_LN2_N: f64 = f64::from_bits(0x4067_1547_652b_82fe);
/// −ln 2/128 in two parts, the first with trailing zeros so that k times it is exact.
const NEG_LN2_HI_N: f64 = f64::from_bits(0xbf76_2e42_fefa_0000);
const NEG_LN2_LO_N: f64 = f64::from_bits(0xbd0c_f79a_bc9e_3b3a);
/// 1.5 2^52: adding it rounds to an integer, kept in the sum's low bits.
const SHIFT: f64 = f64::from_bits(0x4338_0000_0000_0000);
/// The polynomial's coefficients for exp(r) − 1 = r + C2 r² + … + C5 r⁵ (Arm: absolute error 1.555 2^-66 on
/// |r| < ln 2/256).
const C2: f64 = f64::from_bits(0x3fdf_ffff_ffff_fdbd);
const C3: f64 = f64::from_bits(0x3fc5_5555_5555_543c);
const C4: f64 = f64::from_bits(0x3fa5_5555_cf17_2b91);
const C5: f64 = f64::from_bits(0x3f81_1111_67a4_d017);
/// |x|'s bits at 2^-54, below which exp(x) rounds to 1 + x and e^x − 1 to x; at 2^-5, below which `expm1` uses Taylor's
/// polynomial; at 512, from which the scale needs care; and at 1024, beyond which exp is 0 or ∞.
const TINY: u64 = 0x3c90_0000_0000_0000;
const NEAR: u64 = 0x3fa0_0000_0000_0000;
const LARGE: u64 = 0x4080_0000_0000_0000;
const HUGE: u64 = 0x4090_0000_0000_0000;
/// 1009 2^52 and 1022 2^52: the exponent shifts of Arm's `specialcase`, with 2^1009 and 2^-1022, which undo them.
const DOWN_1009: u64 = 0x3f10_0000_0000_0000;
const UP_1022: u64 = 0x3fe0_0000_0000_0000;
const TWO_1009: f64 = f64::from_bits(0x7f00_0000_0000_0000);
const TWO_M1022: f64 = f64::from_bits(0x0010_0000_0000_0000);
/// Taylor's coefficients of (e^x − 1 − x)/x², 1/n! for n = 2 to 8. Below 2^-5 the next term is under 2^-58 of x.
const T2: f64 = 1.0 / 2.0;
const T3: f64 = 1.0 / 6.0;
const T4: f64 = 1.0 / 24.0;
const T5: f64 = 1.0 / 120.0;
const T6: f64 = 1.0 / 720.0;
const T7: f64 = 1.0 / 5040.0;
const T8: f64 = 1.0 / 40320.0;

/// 2^(k/128) ≈ H[k] (1 + T[k]) for k in [0, 128): word 2k is T[k]'s bits, and word 2k + 1 is H[k]'s bits less
/// k 2^52/128, so adding k 2^45 puts ⌊k/128⌋ in the exponent. H[k] is 2^(k/128) rounded to the nearest double and T[k]
/// is (2^(k/128) − H[k])/H[k] rounded, which the separate implementation recomputes and checks.
#[rustfmt::skip]
static TABLE: [u64; 256] = [
    0x0000_0000_0000_0000, 0x3ff0_0000_0000_0000,
    0x3c9b_3b4f_1a88_bf6e, 0x3fef_f63d_a9fb_3335,
    0xbc71_6013_9cd8_dc5d, 0x3fef_ec9a_3e77_8061,
    0xbc90_5e7a_1087_66d1, 0x3fef_e315_e86e_7f85,
    0x3c8c_d252_3567_f613, 0x3fef_d9b0_d315_8574,
    0xbc8b_ce80_23f9_8efa, 0x3fef_d06b_29dd_f6de,
    0x3c60_f74e_61e6_c861, 0x3fef_c745_1875_9bc8,
    0x3c90_a3e4_5b33_d399, 0x3fef_be3e_cac6_f383,
    0x3c97_9aa6_5d83_7b6d, 0x3fef_b558_6cf9_890f,
    0x3c8e_b51a_92fd_effc, 0x3fef_ac92_2b72_47f7,
    0x3c3e_be3d_702f_9cd1, 0x3fef_a3ec_32d3_d1a2,
    0xbc6a_0334_8990_6e0b, 0x3fef_9b66_affe_d31b,
    0xbc95_5652_2a2f_bd0e, 0x3fef_9301_d012_5b51,
    0xbc50_80ef_8c4e_ea55, 0x3fef_8abd_c06c_31cc,
    0xbc91_c923_b9d5_f416, 0x3fef_829a_aea9_2de0,
    0x3c80_d3e3_e95c_55af, 0x3fef_7a98_c8a5_8e51,
    0xbc80_1b15_eaa5_9348, 0x3fef_72b8_3c7d_517b,
    0xbc8f_1ff0_55de_323d, 0x3fef_6af9_388c_8dea,
    0x3c8b_898c_3f13_53bf, 0x3fef_635b_eb6f_cb75,
    0xbc96_d99c_7611_eb26, 0x3fef_5be0_8404_5cd4,
    0x3c9a_ecf7_3e3a_2f60, 0x3fef_5487_3168_b9aa,
    0xbc8f_e782_cb86_389d, 0x3fef_4d50_22fc_d91d,
    0x3c8a_6f41_44a6_c38d, 0x3fef_463b_8862_8cd6,
    0x3c80_7a05_b0e4_047d, 0x3fef_3f49_917d_dc96,
    0x3c96_8efd_e3a8_a894, 0x3fef_387a_6e75_6238,
    0x3c87_5e18_f274_487d, 0x3fef_31ce_4fb2_a63f,
    0x3c80_472b_981f_e7f2, 0x3fef_2b45_65e2_7cdd,
    0xbc96_b87b_3f71_085e, 0x3fef_24df_e1f5_6381,
    0x3c82_f7e1_6d09_ab31, 0x3fef_1e9d_f51f_dee1,
    0xbc3d_219b_1a6f_bffa, 0x3fef_187f_d0da_d990,
    0x3c8b_3782_720c_0ab4, 0x3fef_1285_a6e4_030b,
    0x3c6e_1492_89ce_cb8f, 0x3fef_0caf_a93e_2f56,
    0x3c83_4d75_4db0_abb6, 0x3fef_06fe_0a31_b715,
    0x3c86_4201_e2ac_744c, 0x3fef_0170_fc4c_d831,
    0x3c8f_dd39_5dd3_f84a, 0x3fee_fc08_b264_16ff,
    0xbc86_a380_3b8e_5b04, 0x3fee_f6c5_5f92_9ff1,
    0xbc92_4aed_cc4b_5068, 0x3fee_f1a7_373a_a9cb,
    0xbc99_07f8_1b51_2d8e, 0x3fee_ecae_6d05_d866,
    0xbc71_d1e8_3e94_36d2, 0x3fee_e7db_34e5_9ff7,
    0xbc99_1919_b3ce_1b15, 0x3fee_e32d_c313_a8e5,
    0x3c85_9f48_a72a_4c6d, 0x3fee_dea6_4c12_3422,
    0xbc93_1260_7a28_698a, 0x3fee_da45_04ac_801c,
    0xbc58_a78f_4817_895b, 0x3fee_d60a_21f7_2e2a,
    0xbc7c_2c9b_6749_9a1b, 0x3fee_d1f5_d950_a897,
    0x3c43_63ed_60c2_ac11, 0x3fee_ce08_6061_892d,
    0x3c96_6609_3b06_64ef, 0x3fee_ca41_ed1d_0057,
    0x3c6e_cce1_daa1_0379, 0x3fee_c6a2_b5c1_3cd0,
    0x3c93_ff8e_3f0f_1230, 0x3fee_c32a_f0d7_d3de,
    0x3c76_90ce_bb7a_afb0, 0x3fee_bfda_d536_2a27,
    0x3c93_1dbd_eb54_e077, 0x3fee_bcb2_99fd_dd0d,
    0xbc8f_9434_0071_a38e, 0x3fee_b9b2_769d_2ca7,
    0xbc87_decc_dc93_a349, 0x3fee_b6da_a2cf_6642,
    0xbc78_dec6_bd0f_385f, 0x3fee_b42b_569d_4f82,
    0xbc86_1246_ec7b_5cf6, 0x3fee_b1a4_ca5d_920f,
    0x3c93_3505_18fd_d78e, 0x3fee_af47_36b5_27da,
    0x3c7b_98b7_2f8a_9b05, 0x3fee_ad12_d497_c7fd,
    0x3c90_63e1_e21c_5409, 0x3fee_ab07_dd48_5429,
    0x3c34_c785_5019_c6ea, 0x3fee_a926_8a59_46b7,
    0x3c94_32e6_2b64_c035, 0x3fee_a76f_15ad_2148,
    0xbc8c_e44a_6199_769f, 0x3fee_a5e1_b976_dc09,
    0xbc8c_33c5_3bef_4da8, 0x3fee_a47e_b03a_5585,
    0xbc84_5378_892b_e9ae, 0x3fee_a346_34cc_c320,
    0xbc93_cedd_7856_5858, 0x3fee_a238_8255_2225,
    0x3c57_10aa_807e_1964, 0x3fee_a155_d44c_a973,
    0xbc93_b3ef_bf5e_2228, 0x3fee_a09e_667f_3bcd,
    0xbc6a_12ad_8734_b982, 0x3fee_a012_750b_dabf,
    0xbc63_67ef_b86d_a9ee, 0x3fee_9fb2_3c65_1a2f,
    0xbc80_dc3d_54e0_8851, 0x3fee_9f7d_f951_9484,
    0xbc78_1f64_7e5a_3ecf, 0x3fee_9f75_e8ec_5f74,
    0xbc86_ee4a_c08b_7db0, 0x3fee_9f9a_48a5_8174,
    0xbc86_1932_1e55_e68a, 0x3fee_9feb_5642_67c9,
    0x3c90_9ccb_5e09_d4d3, 0x3fee_a069_4fde_5d3f,
    0xbc7b_32dc_b94d_a51d, 0x3fee_a114_73eb_0187,
    0x3c94_ecfd_5467_c06b, 0x3fee_a1ed_0130_c132,
    0x3c65_ebe1_abd6_6c55, 0x3fee_a2f3_36cf_4e62,
    0xbc88_a1c5_2fb3_cf42, 0x3fee_a427_543e_1a12,
    0xbc93_69b6_f13b_3734, 0x3fee_a589_994c_ce13,
    0xbc80_5e84_3a19_ff1e, 0x3fee_a71a_4623_c7ad,
    0xbc94_d450_d872_576e, 0x3fee_a8d9_9b44_92ed,
    0x3c90_ad67_5b0e_8a00, 0x3fee_aac7_d98a_6699,
    0x3c8d_b72f_c1f0_eab4, 0x3fee_ace5_422a_a0db,
    0xbc65_b660_9cc5_e7ff, 0x3fee_af32_16b5_448c,
    0x3c7b_f683_59f3_5f44, 0x3fee_b1ae_9915_7736,
    0xbc93_091f_a71e_3d83, 0x3fee_b45b_0b91_ffc6,
    0xbc5d_a9b8_8b6c_1e29, 0x3fee_b737_b0cd_c5e5,
    0xbc6c_23f9_7c90_b959, 0x3fee_ba44_cbc8_520f,
    0xbc92_4343_22f4_f9aa, 0x3fee_bd82_9fde_4e50,
    0xbc85_ca6c_d766_8e4b, 0x3fee_c0f1_70ca_07ba,
    0x3c71_affc_2b91_ce27, 0x3fee_c491_82a3_f090,
    0x3c6d_d235_e10a_73bb, 0x3fee_c863_19e3_2323,
    0xbc87_c504_2262_2263, 0x3fee_cc66_7b5d_e565,
    0x3c8b_1c86_e3e2_31d5, 0x3fee_d09b_ec4a_2d33,
    0xbc91_bbd1_d3bc_bb15, 0x3fee_d503_b23e_255d,
    0x3c90_cc31_9cee_31d2, 0x3fee_d99e_1330_b358,
    0x3c84_6984_6e73_5ab3, 0x3fee_de6b_5579_fdbf,
    0xbc82_dfcd_978e_9db4, 0x3fee_e36b_bfd3_f37a,
    0x3c8c_1a77_92cb_3387, 0x3fee_e89f_995a_d3ad,
    0xbc90_7b8f_4ad1_d9fa, 0x3fee_ee07_298d_b666,
    0xbc55_c3d9_56dc_aeba, 0x3fee_f3a2_b84f_15fb,
    0xbc90_a40e_3da6_f640, 0x3fee_f972_8de5_593a,
    0xbc68_d6f4_38ad_9334, 0x3fee_ff76_f2fb_5e47,
    0xbc91_eee2_6b58_8a35, 0x3fef_05b0_30a1_064a,
    0x3c74_ffd7_0a5f_ddcd, 0x3fef_0c1e_904b_c1d2,
    0xbc91_bdfb_fa92_98ac, 0x3fef_12c2_5bd7_1e09,
    0x3c73_6eae_30af_0cb3, 0x3fef_199b_dd85_529c,
    0x3c8e_e332_5c9f_fd94, 0x3fef_20ab_5fff_d07a,
    0x3c84_e08f_d109_59ac, 0x3fef_27f1_2e57_d14b,
    0x3c63_cdaf_384e_1a67, 0x3fef_2f6d_9406_e7b5,
    0x3c67_6b2c_6c92_1968, 0x3fef_3720_dcef_9069,
    0xbc80_8a18_83cc_b5d2, 0x3fef_3f0b_555d_c3fa,
    0xbc8f_ad5d_3fff_fa6f, 0x3fef_472d_4a07_897c,
    0xbc90_0dae_3875_a949, 0x3fef_4f87_080d_89f2,
    0x3c74_a385_a63d_07a7, 0x3fef_5818_dcfb_a487,
    0xbc82_919e_2040_220f, 0x3fef_60e3_16c9_8398,
    0x3c8e_5a50_d5c1_92ac, 0x3fef_69e6_03db_3285,
    0x3c84_3a59_ac01_6b4b, 0x3fef_7321_f301_b460,
    0xbc82_d521_07b4_3e1f, 0x3fef_7c97_337b_9b5f,
    0xbc89_2ab9_3b47_0dc9, 0x3fef_8646_14f5_a129,
    0x3c74_b604_603a_88d3, 0x3fef_902e_e78b_3ff6,
    0x3c83_c5ec_519d_7271, 0x3fef_9a51_fbc7_4c83,
    0xbc8f_f712_8fd3_91f0, 0x3fef_a4af_a2a4_90da,
    0xbc8d_ae98_e223_747d, 0x3fef_af48_2d8e_67f1,
    0x3c8e_c3bc_41aa_2008, 0x3fef_ba1b_ee61_5a27,
    0x3c84_2b94_c3a9_eb32, 0x3fef_c52b_376b_ba97,
    0x3c8a_64a9_31d1_85ee, 0x3fef_d076_5b6e_4540,
    0xbc8e_37ba_e43b_e3ed, 0x3fef_dbfd_ad9c_be14,
    0x3c77_893b_4d91_cd9d, 0x3fef_e7c1_819e_90d8,
    0x3c53_05c1_4160_cc89, 0x3fef_f3c2_2b8f_71f1,
];

/// Word `i` of [`TABLE`], `i` masked to its length, so the lookup cannot miss.
fn word(i: u64) -> u64 {
    TABLE.get(usize::try_from(i & 0xff).unwrap_or(0)).copied().unwrap_or(0)
}

/// |x|'s bits.
fn abs_bits(x: f64) -> u64 {
    x.to_bits() & 0x7fff_ffff_ffff_ffff
}

/// The reduction and the polynomial: (tmp, the scale's bits, k's word), with exp(x) ≈ scale + scale tmp. The scale's
/// bits are a double's for −1023·128 < k < 1024·128, and the word's bit 31 is k's sign.
fn reduce(x: f64) -> (f64, u64, u64) {
    let shifted = INV_LN2_N * x + SHIFT;
    let ki = shifted.to_bits();
    let kd = shifted - SHIFT;
    let r = x + kd * NEG_LN2_HI_N + kd * NEG_LN2_LO_N;
    let index = (ki & 0x7f).wrapping_shl(1);
    let tail = f64::from_bits(word(index));
    let sbits = word(index.wrapping_add(1)).wrapping_add(ki.wrapping_shl(45));
    let r2 = r * r;
    (tail + r + r2 * (C2 + r * C3) + r2 * r2 * (C4 + r * C5), sbits, ki)
}

/// scale + scale tmp, the main path's result.
fn combine(tmp: f64, sbits: u64) -> f64 {
    let scale = f64::from_bits(sbits);
    scale + scale * tmp
}

/// Arm's `specialcase` for k > 0, 512 ≤ x < 1024: the scale's exponent may have overflowed by up to 460, so the scale
/// is taken 2^1009 smaller and the result multiplied back.
fn above(tmp: f64, sbits: u64) -> f64 {
    TWO_1009 * combine(tmp, sbits.wrapping_sub(DOWN_1009))
}

/// Arm's `specialcase` for k < 0, −1024 < x ≤ −512: the scale taken 2^1022 larger and the result multiplied back.
/// Where that result is subnormal (|y| < 1 before the scaling), y is first rounded to its precision there, by adding and
/// taking away 1, so the result is rounded once.
fn below(tmp: f64, sbits: u64) -> f64 {
    let scale = f64::from_bits(sbits.wrapping_add(UP_1022));
    let y = scale + scale * tmp;
    let lo = scale - y + scale * tmp;
    let hi = 1.0 + y;
    let lo = 1.0 - hi + y + lo;
    TWO_M1022 * if y < 1.0 { (hi + lo) - 1.0 } else { y }
}

/// e^x for one number.
fn exp_one(x: f64) -> f64 {
    let size = abs_bits(x);
    if x.is_nan() {
        return x;
    }
    if size >= HUGE {
        return if x.is_sign_negative() { 0.0 } else { f64::INFINITY };
    }
    if size < TINY {
        return 1.0 + x;
    }
    let (tmp, sbits, ki) = reduce(x);
    if size < LARGE {
        combine(tmp, sbits)
    } else if ki & 0x8000_0000 == 0 {
        above(tmp, sbits)
    } else {
        below(tmp, sbits)
    }
}

/// e^x − 1 for one number. From 2^-5 to 512 in magnitude it is (scale − 1) + scale tmp, with scale − 1 summed exactly,
/// so the one rounding that matters is the last; below 2^-5, Taylor's polynomial to x⁸; below 2^-54, x; from 512,
/// e^x less 1, which is e^x or −1.
fn expm1_one(x: f64) -> f64 {
    let size = abs_bits(x);
    if x.is_nan() || size < TINY {
        return x;
    }
    if size < NEAR {
        let p = T6 + x * (T7 + x * T8);
        let p = T4 + x * (T5 + x * p);
        let p = T2 + x * (T3 + x * p);
        return x + x * x * p;
    }
    if size >= LARGE {
        return exp_one(x) - 1.0;
    }
    let (tmp, sbits, _) = reduce(x);
    let scale = f64::from_bits(sbits);
    let (hi, lo) = two_sum(scale, -1.0);
    hi + (lo + scale * tmp)
}

/// e^x, within 0.511 ulp by Arm's bound: the same bits on every target, lane width and build. 0 below about −745.13,
/// ∞ above about 709.78, 1 for ±0, and NaN for NaN.
#[inline(always)]
pub fn exp<R: Real>(x: R) -> R {
    x.map(exp_one)
}

/// e^x − 1, accurate near 0 where e^x − 1 computed as written is not: the same bits on every target, lane width and
/// build. −1 below about −37.4, ∞ above about 709.78, ±0 for ±0, and NaN for NaN.
#[inline(always)]
pub fn expm1<R: Real>(x: R) -> R {
    x.map(expm1_one)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;

    /// How many doubles apart two numbers of the same sign are.
    fn apart(a: f64, b: f64) -> u64 {
        a.to_bits().abs_diff(b.to_bits())
    }

    /// Each number, its neighbours a double either side, and their negatives.
    fn around(edges: &[f64]) -> Vec<f64> {
        let signed: Vec<f64> = edges.iter().flat_map(|e| [*e, -e]).collect();
        signed.into_iter().flat_map(|e| [e, e.next_up(), e.next_down()]).collect()
    }

    #[test]
    fn exp_is_within_an_ulp_of_the_correctly_rounded_value() {
        // From Python's `decimal` at 60 digits, rounded once to a double.
        let known = [
            (1.0, std::f64::consts::E),
            (-1.0, 0.367_879_441_171_442_33),
            (0.5, 1.648_721_270_700_128_2),
            (10.0, 22_026.465_794_806_718),
            (-10.0, 4.539_992_976_248_485_4e-05),
            (100.0, 2.688_117_141_816_135_6e+43),
            (700.0, 1.014_232_054_735_004_5e+304),
            (709.78, 1.792_822_794_394_515_5e+308),
            (-708.5, 2.006_132_305_331_306e-308),
            (-740.0, 4.2e-322),
            (3.0e-10, 1.000_000_000_3),
            (std::f64::consts::LN_2, 2.0),
        ];
        for (x, want) in known {
            assert!(apart(exp(x), want) <= 1, "exp({x}) = {}, not {want}", exp(x));
        }
    }

    #[test]
    fn exp_meets_its_limits() {
        assert_eq!([exp(0.0), exp(-0.0), exp(1e-300), exp(-1e-300)], [1.0; 4]);
        assert_eq!([exp(f64::INFINITY), exp(f64::NEG_INFINITY)], [f64::INFINITY, 0.0]);
        assert!(exp(f64::NAN).is_nan());
        assert!(exp(709.78).is_finite() && exp(709.79) == f64::INFINITY && exp(1023.9) == f64::INFINITY);
        assert_eq!((exp(-745.2), exp(-1023.9), exp(-1024.0)), (0.0, 0.0, 0.0));
        assert_eq!(exp(-745.1).to_bits(), 1, "the smallest subnormal");
        assert_eq!(exp(f64::from_bits(TINY)), 1.0 + f64::from_bits(TINY));
    }

    #[test]
    fn the_paths_meet_where_they_change() {
        // 2^-54, 512 and 1024: each side of every edge of `exp_one`'s paths, against its neighbour in the other path.
        for x in around(&[f64::from_bits(TINY), f64::from_bits(LARGE), f64::from_bits(HUGE)]) {
            let (at, next) = (exp(x), exp(x.next_up()));
            assert!(at <= next, "exp falls from {x} to the next double: {at} {next}");
        }
        let large = f64::from_bits(LARGE);
        assert!(apart(exp(large), exp(large.next_down())) <= 1 << 41, "512 sits on both sides of the scale's care");
        assert!(apart(exp(-large), exp(-large.next_down())) <= 1 << 41);
    }

    #[test]
    fn exp_rounds_once_where_another_path_would_round_twice() {
        // From Python's `decimal` at 80 digits, rounded once to a double. At −708.45 and −711.21 the result is
        // subnormal, and e^x rounded to a double and then to the subnormal spacing misses by an ulp; at −708 it is
        // normal, and rounding it at the subnormal spacing misses; at 8.4e-5 the scale's care, taken below 512, makes
        // the scale's product with tmp subnormal and rounds it on its own.
        let known = [
            (8.4e-5, 1.000_084_003_528_098_7),
            (-708.0, 3.307_553_003_638_408e-308),
            (-708.45, 2.108_988_908_100_906_4e-308),
            (-711.21, 1.334_816_374_445_75e-309),
        ];
        for (x, want) in known {
            assert_eq!(exp(x), want, "exp({x})");
        }
    }

    #[test]
    fn expm1_is_within_an_ulp_near_zero_and_beyond() {
        let known = [
            (1.0e-10, 1.000_000_000_05e-10),
            (-1.0e-10, -9.999_999_999_500_001e-11),
            (0.03, 0.030_454_533_953_516_855),
            (-0.03, -0.029_554_466_451_491_82),
            (0.031_3, 0.031_794_995_959_178_38),
            (0.031_2, 0.031_691_821_618_385_48),
            (std::f64::consts::LN_2, 1.0),
            (1.0, 1.718_281_828_459_045_3),
            (-1.0, -0.632_120_558_828_557_7),
            (30.0, 10_686_474_581_523.463),
            (-30.0, -0.999_999_999_999_906_4),
            (600.0, 3.773_020_300_929_939_7e+260),
        ];
        for (x, want) in known {
            assert!(apart(expm1(x), want) <= 1, "expm1({x}) = {}, not {want}", expm1(x));
        }
    }

    #[test]
    fn expm1_meets_its_limits() {
        assert_eq!([expm1(0.0).to_bits(), expm1(-0.0).to_bits()], [0.0f64.to_bits(), (-0.0f64).to_bits()]);
        assert_eq!([expm1(1e-300), expm1(-5e-324)], [1e-300, -5e-324]);
        assert_eq!(
            [expm1(f64::INFINITY), expm1(f64::NEG_INFINITY), expm1(-600.0), expm1(-40.0)],
            [f64::INFINITY, -1.0, -1.0, -1.0]
        );
        assert!(expm1(f64::NAN).is_nan() && expm1(709.79) == f64::INFINITY);
        let near = f64::from_bits(NEAR);
        for x in around(&[near]) {
            let below = if x.abs() < near { x.next_up() } else { x.next_down() };
            assert!(apart(expm1(x), expm1(below)) <= 2, "the paths meet at 2^-5: {x}");
        }
    }

    #[test]
    fn lanes_give_each_lanes_own_bits() {
        let x = [1.0, -745.1, f64::NAN, 3e-10, 600.0, -0.03, f64::INFINITY, 0.031_25];
        let bits = |v: [f64; 8]| v.map(crate::canonical_bits);
        assert_eq!(bits(exp(Lanes::new(x)).to_array()), bits(x.map(exp)));
        assert_eq!(bits(expm1(Lanes::new(x)).to_array()), bits(x.map(expm1)));
    }
}
