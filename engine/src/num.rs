//! Numbers: exact rationals (BigRational) while every step stays exact, f64 otherwise.
//! Exactness is contagious the other way: once a float enters, the result is a float.

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::fmt::Write;

#[derive(Clone, Debug)]
pub enum Num {
    Q(BigRational),
    R(f64),
}

/// Limits that keep a student's typo (2^99999999) from hanging the engine.
const MAX_EXP: u64 = 100_000;
const MAX_BITS: u64 = 1 << 20;

pub type NResult = Result<Num, String>;

impl Num {
    pub fn int(i: i64) -> Num {
        Num::Q(BigRational::from_integer(BigInt::from(i)))
    }
    pub fn ratio(n: i64, d: i64) -> Num {
        Num::Q(BigRational::new(BigInt::from(n), BigInt::from(d)))
    }
    pub fn zero() -> Num {
        Num::int(0)
    }
    pub fn one() -> Num {
        Num::int(1)
    }

    /// A decimal literal ("3.25", ".5", "12") as an exact rational.
    pub fn parse_decimal(s: &str) -> Option<Num> {
        let (ip, fp) = match s.split_once('.') {
            Some((a, b)) => (a, b),
            None => (s, ""),
        };
        if ip.is_empty() && fp.is_empty() {
            return None;
        }
        let digits = format!("{ip}{fp}");
        let n: BigInt = if digits.is_empty() { BigInt::zero() } else { digits.parse().ok()? };
        let d = num_traits::pow(BigInt::from(10), fp.len());
        Some(Num::Q(BigRational::new(n, d)))
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Num::Q(_))
    }
    pub fn to_f64(&self) -> f64 {
        match self {
            Num::Q(q) => q.to_f64().unwrap_or(f64::NAN),
            Num::R(x) => *x,
        }
    }
    pub fn is_zero(&self) -> bool {
        match self {
            Num::Q(q) => q.is_zero(),
            Num::R(x) => *x == 0.0,
        }
    }
    pub fn is_integer(&self) -> bool {
        match self {
            Num::Q(q) => q.is_integer(),
            Num::R(x) => x.fract() == 0.0 && x.is_finite(),
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Num::Q(q) if q.is_integer() => q.to_integer().to_i64(),
            Num::R(x) if x.fract() == 0.0 && x.abs() < 9.0e15 => Some(*x as i64),
            _ => None,
        }
    }
    pub fn sign(&self) -> i32 {
        match self {
            Num::Q(q) => {
                if q.is_zero() {
                    0
                } else if q.is_positive() {
                    1
                } else {
                    -1
                }
            }
            Num::R(x) => {
                if *x > 0.0 {
                    1
                } else if *x < 0.0 {
                    -1
                } else {
                    0
                }
            }
        }
    }

    fn float(x: f64) -> NResult {
        if x.is_nan() {
            Err("undefined".into())
        } else {
            Ok(Num::R(x))
        }
    }

    pub fn add(&self, o: &Num) -> NResult {
        match (self, o) {
            (Num::Q(a), Num::Q(b)) => Ok(Num::Q(a + b)),
            _ => Num::float(self.to_f64() + o.to_f64()),
        }
    }
    pub fn sub(&self, o: &Num) -> NResult {
        match (self, o) {
            (Num::Q(a), Num::Q(b)) => Ok(Num::Q(a - b)),
            _ => Num::float(self.to_f64() - o.to_f64()),
        }
    }
    pub fn mul(&self, o: &Num) -> NResult {
        match (self, o) {
            (Num::Q(a), Num::Q(b)) => Ok(Num::Q(a * b)),
            _ => Num::float(self.to_f64() * o.to_f64()),
        }
    }
    pub fn div(&self, o: &Num) -> NResult {
        if o.is_zero() {
            return Err("division by zero".into());
        }
        match (self, o) {
            (Num::Q(a), Num::Q(b)) => Ok(Num::Q(a / b)),
            _ => Num::float(self.to_f64() / o.to_f64()),
        }
    }
    pub fn neg(&self) -> Num {
        match self {
            Num::Q(a) => Num::Q(-a),
            Num::R(x) => Num::R(-x),
        }
    }
    pub fn abs(&self) -> Num {
        match self {
            Num::Q(a) => Num::Q(a.abs()),
            Num::R(x) => Num::R(x.abs()),
        }
    }

    /// a^b: exact for rational a and integer b, and for rational roots that come out exact (8^(2/3) = 4).
    pub fn pow(&self, e: &Num) -> NResult {
        if let (Num::Q(a), Num::Q(b)) = (self, e) {
            if b.is_integer() {
                let n = b.to_integer();
                let k = n.abs().to_u64().filter(|k| *k <= MAX_EXP);
                if let Some(k) = k {
                    if a.is_zero() && n.is_negative() {
                        return Err("division by zero".into());
                    }
                    let bits = (a.numer().bits().max(a.denom().bits())).saturating_mul(k);
                    if bits <= MAX_BITS {
                        let p = num_traits::pow(a.clone(), k as usize);
                        return Ok(Num::Q(if n.is_negative() { p.recip() } else { p }));
                    }
                }
            } else if let Some(r) = exact_root(a, b) {
                return Ok(Num::Q(r));
            }
        }
        let (x, y) = (self.to_f64(), e.to_f64());
        if x < 0.0 && y.fract() != 0.0 {
            // odd roots of negatives are real: (-8)^(1/3) = -2
            if let Num::Q(b) = e {
                if b.denom().is_odd() {
                    let r = (-x).powf(y);
                    return Num::float(if b.numer().is_odd() { -r } else { r });
                }
            }
            return Err("undefined (even root of a negative number)".into());
        }
        Num::float(x.powf(y))
    }

    /// Plain-text form: "3/4", "-2", "0.3333333333".
    pub fn plain(&self, digits: usize) -> String {
        match self {
            Num::Q(q) if q.is_integer() => q.to_integer().to_string(),
            Num::Q(q) => format!("{}/{}", q.numer(), q.denom()),
            Num::R(x) => fmt_float(*x, digits),
        }
    }
    /// Decimal approximation, or None when it would just repeat the exact form.
    pub fn approx(&self, digits: usize) -> Option<String> {
        match self {
            Num::Q(q) if q.is_integer() => None,
            Num::Q(_) => Some(fmt_float(self.to_f64(), digits)),
            Num::R(_) => None,
        }
    }
}

/// Exact p/q-th root of a rational when the numerator and denominator are perfect powers.
fn exact_root(a: &BigRational, e: &BigRational) -> Option<BigRational> {
    let q = e.denom().to_u32().filter(|q| *q <= 64)?;
    let p = e.numer().to_i64().filter(|p| p.unsigned_abs() <= MAX_EXP)?;
    if a.is_negative() && q % 2 == 0 {
        return None;
    }
    let neg = a.is_negative();
    let n = iroot(&a.numer().abs(), q)?;
    let d = iroot(a.denom(), q)?;
    let mut r = BigRational::new(if neg { -n } else { n }, d);
    if r.is_zero() && p < 0 {
        return None;
    }
    r = num_traits::pow(r, p.unsigned_abs() as usize);
    Some(if p < 0 { r.recip() } else { r })
}

/// Integer q-th root when exact.
pub fn iroot(n: &BigInt, q: u32) -> Option<BigInt> {
    if n.is_zero() || n.is_one() {
        return Some(n.clone());
    }
    let r = n.nth_root(q);
    (num_traits::pow(r.clone(), q as usize) == *n).then_some(r)
}

/// Up to `digits` significant digits, trailing zeros trimmed, scientific outside 1e-6..1e15.
pub fn fmt_float(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "undefined".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "∞".into() } else { "−∞".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let digits = digits.clamp(1, 17);
    let ax = x.abs();
    if !(1e-6..1e15).contains(&ax) {
        let s = format!("{:.*e}", digits - 1, x);
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = trim_zeros(m);
        return format!("{m}×10^{e}");
    }
    let mag = ax.log10().floor() as i32;
    let dec = (digits as i32 - 1 - mag).clamp(0, 17) as usize;
    let mut s = String::new();
    let _ = write!(s, "{:.*}", dec, x);
    trim_zeros(&s)
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_arithmetic() {
        let a = Num::ratio(1, 3);
        let b = Num::ratio(1, 6);
        assert_eq!(a.add(&b).unwrap().plain(10), "1/2");
        assert_eq!(Num::int(2).pow(&Num::int(10)).unwrap().plain(10), "1024");
        assert_eq!(Num::int(2).pow(&Num::int(-2)).unwrap().plain(10), "1/4");
        assert_eq!(Num::int(8).pow(&Num::ratio(2, 3)).unwrap().plain(10), "4");
        assert_eq!(Num::int(-8).pow(&Num::ratio(1, 3)).unwrap().plain(10), "-2");
        assert!(Num::int(1).div(&Num::zero()).is_err());
        assert_eq!(Num::parse_decimal("0.25").unwrap().plain(10), "1/4");
    }

    #[test]
    fn floats() {
        assert_eq!(fmt_float(1.0 / 3.0, 10), "0.3333333333");
        assert_eq!(fmt_float(2.0f64.sqrt(), 6), "1.41421");
        assert_eq!(fmt_float(1e20, 4), "1×10^20");
        assert_eq!(Num::int(2).pow(&Num::ratio(1, 2)).unwrap().plain(8), "1.4142136");
    }
}
