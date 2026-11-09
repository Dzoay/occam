//! One maths implementation: nothing computes a result that Rust does not pin down (L23).

pub fn probe(x: f64, y: f64) {
    let _ = x.powi(2); // fires: clippy::disallowed_methods f64::powi
    let _ = x.powf(y); // fires: clippy::disallowed_methods f64::powf
    let _ = x.exp(); // fires: clippy::disallowed_methods f64::exp
    let _ = x.exp2(); // fires: clippy::disallowed_methods f64::exp2
    let _ = x.ln(); // fires: clippy::disallowed_methods f64::ln
    let _ = x.log(y); // fires: clippy::disallowed_methods f64::log
    let _ = x.log2(); // fires: clippy::disallowed_methods f64::log2
    let _ = x.log10(); // fires: clippy::disallowed_methods f64::log10
    let _ = x.cbrt(); // fires: clippy::disallowed_methods f64::cbrt
    let _ = x.hypot(y); // fires: clippy::disallowed_methods f64::hypot
    let _ = x.sin(); // fires: clippy::disallowed_methods f64::sin
    let _ = x.cos(); // fires: clippy::disallowed_methods f64::cos
    let _ = x.tan(); // fires: clippy::disallowed_methods f64::tan
    let _ = x.asin(); // fires: clippy::disallowed_methods f64::asin
    let _ = x.acos(); // fires: clippy::disallowed_methods f64::acos
    let _ = x.atan(); // fires: clippy::disallowed_methods f64::atan
    let _ = x.atan2(y); // fires: clippy::disallowed_methods f64::atan2
    let _ = x.sin_cos(); // fires: clippy::disallowed_methods f64::sin_cos
    let _ = x.exp_m1(); // fires: clippy::disallowed_methods f64::exp_m1
    let _ = x.ln_1p(); // fires: clippy::disallowed_methods f64::ln_1p
    let _ = x.sinh(); // fires: clippy::disallowed_methods f64::sinh
    let _ = x.cosh(); // fires: clippy::disallowed_methods f64::cosh
    let _ = x.tanh(); // fires: clippy::disallowed_methods f64::tanh
    let _ = x.asinh(); // fires: clippy::disallowed_methods f64::asinh
    let _ = x.acosh(); // fires: clippy::disallowed_methods f64::acosh
    let _ = x.atanh(); // fires: clippy::disallowed_methods f64::atanh
    let _ = x.to_degrees(); // fires: clippy::disallowed_methods f64::to_degrees
    let _ = x.to_radians(); // fires: clippy::disallowed_methods f64::to_radians
    let _ = x.max(y); // fires: clippy::disallowed_methods f64::max
    let _ = x.min(y); // fires: clippy::disallowed_methods f64::min
    let _ = x.clamp(y, y); // fires: clippy::disallowed_methods f64::clamp
    let _ = x.algebraic_add(y); // fires: clippy::disallowed_methods f64::algebraic_add
    let _ = x.algebraic_sub(y); // fires: clippy::disallowed_methods f64::algebraic_sub
    let _ = x.algebraic_mul(y); // fires: clippy::disallowed_methods f64::algebraic_mul
    let _ = x.algebraic_div(y); // fires: clippy::disallowed_methods f64::algebraic_div
    let _ = x.algebraic_rem(y); // fires: clippy::disallowed_methods f64::algebraic_rem
    let _ = x.mul_add(y, y); // fires: clippy::disallowed_methods f64::mul_add
}
