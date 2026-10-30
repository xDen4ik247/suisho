//! Polynomials in one variable with rational coefficients: built from the syntax tree (+ − · ^n, division by a
//! constant), arithmetic, Horner/synthetic division, rational-root candidates, and real roots for the rest.

use crate::parse::Ast;
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    /// c[i] is the coefficient of x^i; no trailing zeros (the zero polynomial is empty)
    pub c: Vec<BigRational>,
}

pub fn q(n: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(n))
}

impl Poly {
    pub fn zero() -> Poly {
        Poly { c: vec![] }
    }
    pub fn constant(r: BigRational) -> Poly {
        Poly { c: vec![r] }.trim()
    }
    pub fn x() -> Poly {
        Poly { c: vec![q(0), q(1)] }
    }
    fn trim(mut self) -> Poly {
        while self.c.last().is_some_and(|x| x.is_zero()) {
            self.c.pop();
        }
        self
    }
    pub fn deg(&self) -> i32 {
        self.c.len() as i32 - 1
    }
    pub fn lead(&self) -> BigRational {
        self.c.last().cloned().unwrap_or_else(|| q(0))
    }
    pub fn coef(&self, i: usize) -> BigRational {
        self.c.get(i).cloned().unwrap_or_else(|| q(0))
    }
    pub fn add(&self, o: &Poly) -> Poly {
        let n = self.c.len().max(o.c.len());
        Poly { c: (0..n).map(|i| self.coef(i) + o.coef(i)).collect() }.trim()
    }
    pub fn neg(&self) -> Poly {
        Poly { c: self.c.iter().map(|x| -x).collect() }
    }
    pub fn sub(&self, o: &Poly) -> Poly {
        self.add(&o.neg())
    }
    pub fn mul(&self, o: &Poly) -> Poly {
        if self.c.is_empty() || o.c.is_empty() {
            return Poly::zero();
        }
        let mut c = vec![q(0); self.c.len() + o.c.len() - 1];
        for (i, a) in self.c.iter().enumerate() {
            for (j, b) in o.c.iter().enumerate() {
                c[i + j] += a * b;
            }
        }
        Poly { c }.trim()
    }
    pub fn scale(&self, k: &BigRational) -> Poly {
        Poly { c: self.c.iter().map(|x| x * k).collect() }.trim()
    }
    pub fn pow(&self, n: u32) -> Poly {
        let mut r = Poly::constant(q(1));
        for _ in 0..n {
            r = r.mul(self);
        }
        r
    }
    pub fn eval(&self, x: &BigRational) -> BigRational {
        let mut acc = q(0);
        for a in self.c.iter().rev() {
            acc = acc * x + a;
        }
        acc
    }
    pub fn eval_f(&self, x: f64) -> f64 {
        self.c.iter().rev().fold(0.0, |acc, a| acc * x + a.to_f64().unwrap_or(f64::NAN))
    }
    /// Synthetic division by (x − r): (quotient, remainder, the Horner row of partial values).
    pub fn div_linear(&self, r: &BigRational) -> (Poly, BigRational, Vec<BigRational>) {
        let mut row = Vec::new();
        let mut acc = q(0);
        for a in self.c.iter().rev() {
            acc = acc * r + a;
            row.push(acc.clone());
        }
        let rem = row.pop().unwrap_or_else(|| q(0));
        let mut qc: Vec<BigRational> = row.clone();
        qc.reverse();
        row.push(rem.clone());
        (Poly { c: qc }.trim(), rem, row)
    }
    /// The same polynomial with integer coefficients (times the lcm of the denominators / content).
    pub fn integer_form(&self) -> (Poly, BigRational) {
        let l = self.c.iter().fold(BigInt::one(), |acc, x| acc.lcm(x.denom()));
        let p = self.scale(&BigRational::from_integer(l.clone()));
        let g = p.c.iter().fold(BigInt::zero(), |acc, x| acc.gcd(&x.to_integer()));
        let g = if g.is_zero() { BigInt::one() } else { g };
        let k = BigRational::new(l, g);
        (self.scale(&k), k)
    }
    /// Candidates ±p/q for rational roots (p | constant term, q | leading coefficient), small ones first.
    pub fn rational_candidates(&self) -> Vec<BigRational> {
        let (p, _) = self.integer_form();
        let a0 = p.coef(0).to_integer().abs();
        let an = p.lead().to_integer().abs();
        if a0.is_zero() {
            return vec![q(0)];
        }
        let divs = |n: &BigInt| -> Vec<BigInt> {
            let n = n.to_u64().unwrap_or(0);
            if n == 0 || n > 10_000_000_000 {
                return vec![];
            }
            let mut d = Vec::new();
            let mut i = 1u64;
            while i * i <= n {
                if n % i == 0 {
                    d.push(BigInt::from(i));
                    if i * i != n {
                        d.push(BigInt::from(n / i));
                    }
                }
                i += 1;
            }
            d
        };
        let mut out: Vec<BigRational> = Vec::new();
        for a in divs(&a0) {
            for b in divs(&an) {
                let r = BigRational::new(a.clone(), b);
                for c in [r.clone(), -r] {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.abs().cmp(&b.abs()).then(b.cmp(a)));
        out
    }
    /// Real roots (f64) of any degree: isolate sign changes on a fine grid + bisection, plus double roots via the derivative.
    pub fn real_roots(&self) -> Vec<f64> {
        if self.deg() < 1 {
            return vec![];
        }
        // Cauchy bound
        let an = self.lead().to_f64().unwrap_or(1.0).abs();
        let b = 1.0 + self.c.iter().take(self.c.len() - 1).map(|a| a.to_f64().unwrap_or(0.0).abs() / an).fold(0.0, f64::max);
        let d = self.derivative();
        let mut pts = vec![-b, b];
        if d.deg() >= 1 {
            pts.extend(d.real_roots());
        }
        pts.sort_by(f64::total_cmp);
        let mut roots = Vec::new();
        for w in pts.windows(2) {
            let (mut lo, mut hi) = (w[0], w[1]);
            let (flo, fhi) = (self.eval_f(lo), self.eval_f(hi));
            if flo == 0.0 {
                roots.push(lo);
                continue;
            }
            if flo.signum() == fhi.signum() {
                continue;
            }
            for _ in 0..200 {
                let m = (lo + hi) / 2.0;
                if (self.eval_f(m) < 0.0) == (flo < 0.0) {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            roots.push((lo + hi) / 2.0);
        }
        // touching roots (double): extrema of the polynomial where it is ~0
        for &p in &pts {
            if self.eval_f(p).abs() < 1e-10 * (1.0 + an) && !roots.iter().any(|r| (r - p).abs() < 1e-7) {
                roots.push(p);
            }
        }
        if let Some(&last) = pts.last() {
            if self.eval_f(last) == 0.0 && !roots.contains(&last) {
                roots.push(last);
            }
        }
        roots.sort_by(f64::total_cmp);
        roots.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        roots
    }
    pub fn derivative(&self) -> Poly {
        Poly { c: self.c.iter().enumerate().skip(1).map(|(i, a)| a * q(i as i64)).collect() }.trim()
    }
}

/// The polynomial in `var` that the expression is, if it is one (other letters must be numbers in `consts`).
pub fn from_ast(a: &Ast, var: &str, consts: &dyn Fn(&str) -> Option<BigRational>) -> Option<Poly> {
    Some(match a {
        Ast::Num(s) => match crate::num::Num::parse_decimal(s)? {
            crate::num::Num::Q(r) => Poly::constant(r),
            _ => return None,
        },
        Ast::Var(n) if n == var => Poly::x(),
        Ast::Var(n) => Poly::constant(consts(n)?),
        Ast::Neg(x) => from_ast(x, var, consts)?.neg(),
        Ast::Add(x, y) => from_ast(x, var, consts)?.add(&from_ast(y, var, consts)?),
        Ast::Sub(x, y) => from_ast(x, var, consts)?.sub(&from_ast(y, var, consts)?),
        Ast::Mul(x, y, _) => from_ast(x, var, consts)?.mul(&from_ast(y, var, consts)?),
        Ast::Div(x, y) => {
            let d = from_ast(y, var, consts)?;
            if d.deg() != 0 {
                return None;
            }
            from_ast(x, var, consts)?.scale(&d.lead().recip())
        }
        Ast::Pow(x, y) => {
            let e = match &**y {
                Ast::Num(s) => s.parse::<u32>().ok()?,
                _ => return None,
            };
            if e > 64 {
                return None;
            }
            from_ast(x, var, consts)?.pow(e)
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        // (x − 1)(x − 2)(x − 3)
        let p = Poly { c: vec![q(-6), q(11), q(-6), q(1)] };
        assert_eq!(p.eval(&q(2)), q(0));
        let (quo, rem, _) = p.div_linear(&q(1));
        assert_eq!(rem, q(0));
        assert_eq!(quo.c, vec![q(6), q(-5), q(1)]);
        let r = p.real_roots();
        assert_eq!(r.len(), 3);
        assert!((r[2] - 3.0).abs() < 1e-9);
        assert!(p.rational_candidates().contains(&q(3)));
        // double root x² − 2x + 1
        let d = Poly { c: vec![q(1), q(-2), q(1)] };
        assert_eq!(d.real_roots().len(), 1);
    }
}

/// The polynomial in `x` that a symbolic expression is (rational coefficients only).
pub fn from_ast_e(e: &crate::sym::E, x: &str) -> Option<Poly> {
    use crate::sym::E;
    Some(match e {
        E::Q(r) => Poly::constant(r.clone()),
        E::Sym(s) if s == x => Poly::x(),
        E::Add(v) => v.iter().map(|z| from_ast_e(z, x)).collect::<Option<Vec<_>>>()?.iter().fold(Poly::zero(), |a, b| a.add(b)),
        E::Mul(v) => v.iter().map(|z| from_ast_e(z, x)).collect::<Option<Vec<_>>>()?.iter().fold(Poly::constant(q(1)), |a, b| a.mul(b)),
        E::Pow(b, p) => {
            let n = p.is_q().filter(|r| r.is_integer() && !r.is_negative())?.to_integer().to_u32().filter(|n| *n <= 64)?;
            from_ast_e(b, x)?.pow(n)
        }
        _ => return None,
    })
}
