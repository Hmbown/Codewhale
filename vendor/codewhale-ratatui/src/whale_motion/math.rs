//! The few JavaScript numeric semantics the oracle depends on, so the port
//! reproduces its arithmetic rather than approximating it.

pub const TAU: f64 = std::f64::consts::PI * 2.;
pub const D2R: f64 = std::f64::consts::PI / 180.;

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// `Math.max(a, Math.min(b, v))`, NaN-propagating like JavaScript.
pub fn clamp(v: f64, a: f64, b: f64) -> f64 {
    if v.is_nan() {
        return v;
    }
    let low = if b < v { b } else { v };
    if a > low { a } else { low }
}

pub fn smooth(a: f64, b: f64, v: f64) -> f64 {
    let t = clamp((v - a) / (b - a), 0., 1.);
    t * t * (3. - 2. * t)
}

/// V8's `Math.hypot`: scale by the largest magnitude, then a compensated sum.
pub fn hypot(values: &[f64]) -> f64 {
    let mut max = 0f64;
    for value in values {
        let abs = value.abs();
        if abs.is_nan() {
            return f64::NAN;
        }
        if abs > max {
            max = abs;
        }
    }
    if max == f64::INFINITY {
        return f64::INFINITY;
    }
    if max == 0. {
        return 0.;
    }
    let mut sum = 0.;
    let mut compensation = 0.;
    for value in values {
        let n = value.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

pub fn hypot2(a: f64, b: f64) -> f64 {
    hypot(&[a, b])
}

/// `Math.round`: halves round toward positive infinity.
pub fn round(v: f64) -> f64 {
    let floor = v.floor();
    if v - floor >= 0.5 { floor + 1. } else { floor }
}

/// `+n.toFixed(3)` then `String(n)`: the geometry receipt's number format.
pub fn fixed3(v: f64) -> String {
    let negative = v < 0.;
    let abs = v.abs();
    // toFixed rounds exact decimal ties away from zero; Rust's formatter
    // rounds them to even. A tie at three places is a multiple of 1/16 that
    // is not a multiple of 1/8, which binary represents exactly.
    let sixteenths = abs * 16.;
    let text = if sixteenths.fract() == 0. && sixteenths % 2. == 1. {
        format!("{:.3}", abs + 0.0001)
    } else {
        format!("{abs:.3}")
    };
    let rounded: f64 = text.parse().unwrap();
    if rounded == 0. {
        return "0".into();
    }
    let body = format!("{rounded}");
    if negative { format!("-{body}") } else { body }
}

/// mulberry32, bit-exact with acting.js.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }
    pub fn draw(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn javascript_numeric_semantics() {
        assert_eq!(round(2.5), 3.);
        assert_eq!(round(-2.5), -2.);
        assert_eq!(round(-2.6), -3.);
        assert_eq!(fixed3(0.0625), "0.063");
        assert_eq!(fixed3(-0.0625), "-0.063");
        assert_eq!(fixed3(-0.0001), "0");
        assert_eq!(fixed3(2.), "2");
        assert_eq!(fixed3(1.23456), "1.235");
        assert_eq!(fixed3(-12.5), "-12.5");
        assert_eq!(hypot2(3., 4.), 5.);
        assert_eq!(clamp(5., 0., 1.), 1.);
        // mulberry32(11), first draws, from node.
        let mut rng = Rng::new(11);
        let first = rng.draw();
        assert!((0. ..1.).contains(&first));
    }
}
