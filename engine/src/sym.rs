//! Symbolic expressions for exact answers: rationals, π, e, square/cube roots, symbols, sums,
//! products and powers in a canonical form. `simplify` collects like terms (2x + 3x = 5x), merges powers
//! (x·x² = x³), pulls perfect powers out of roots (√8 = 2√2, ∛54 = 3∛2), rationalises 1/√2 = √2/2, and knows
//! the exact sin/cos/tan of multiples of π/12, π/10, π/8, π/6, π/4.

use crate::parse::Ast;
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub enum E {
    Q(BigRational),
    Sym(String),
    Pi,
    Euler,
    Add(Vec<E>),
    Mul(Vec<E>),
    Pow(Box<E>, Box<E>),
    Fn(String, Vec<E>),
}

pub fn q(n: i64) -> E {
    E::Q(BigRational::from_integer(BigInt::from(n)))
}
pub fn qr(n: i64, d: i64) -> E {
    E::Q(BigRational::new(BigInt::from(n), BigInt::from(d)))
}

impl E {
    pub fn is_q(&self) -> Option<&BigRational> {
        if let E::Q(r) = self {
            Some(r)
        } else {
            None
        }
    }
    pub fn is_zero(&self) -> bool {
        matches!(self, E::Q(r) if r.is_zero())
    }
    pub fn is_one(&self) -> bool {
        matches!(self, E::Q(r) if r.is_one())
    }
    pub fn has_sym(&self) -> bool {
        match self {
            E::Sym(_) => true,
            E::Add(v) | E::Mul(v) | E::Fn(_, v) => v.iter().any(E::has_sym),
            E::Pow(a, b) => a.has_sym() || b.has_sym(),
            _ => false,
        }
    }
    pub fn to_f64(&self) -> f64 {
        match self {
            E::Q(r) => r.to_f64().unwrap_or(f64::NAN),
            E::Pi => std::f64::consts::PI,
            E::Euler => std::f64::consts::E,
            E::Sym(_) => f64::NAN,
            E::Add(v) => v.iter().map(E::to_f64).sum(),
            E::Mul(v) => v.iter().map(E::to_f64).product(),
            E::Pow(a, b) => {
                let (x, y) = (a.to_f64(), b.to_f64());
                if x < 0.0 {
                    if let E::Q(r) = &**b {
                        if r.denom().is_odd() {
                            let v = (-x).powf(y);
                            return if r.numer().is_odd() { -v } else { v };
                        }
                    }
                }
                x.powf(y)
            }
            E::Fn(f, a) => {
                let x = a.first().map(E::to_f64).unwrap_or(f64::NAN);
                match f.as_str() {
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    "ln" => x.ln(),
                    "abs" => x.abs(),
                    _ => f64::NAN,
                }
            }
        }
    }
}

fn add(v: Vec<E>) -> E {
    simplify(&E::Add(v))
}
fn mul(v: Vec<E>) -> E {
    simplify(&E::Mul(v))
}
fn pow(a: E, b: E) -> E {
    simplify(&E::Pow(Box::new(a), Box::new(b)))
}

/// Ast → E for the exact part of the language (numbers, π, e, symbols, + − · / ^, √, a few functions).
/// None for anything else (matrices, lists, relations…): the numeric evaluator handles those.
pub fn from_ast(a: &Ast, degrees: bool) -> Option<E> {
    Some(match a {
        Ast::Num(s) => match crate::num::Num::parse_decimal(s)? {
            crate::num::Num::Q(r) => E::Q(r),
            _ => return None,
        },
        Ast::Var(n) => match n.as_str() {
            "pi" => E::Pi,
            "e" => E::Euler,
            "∞" => return None,
            n => E::Sym(n.to_string()),
        },
        Ast::Neg(x) => mul(vec![q(-1), from_ast(x, degrees)?]),
        Ast::Add(x, y) => add(vec![from_ast(x, degrees)?, from_ast(y, degrees)?]),
        Ast::Sub(x, y) => add(vec![from_ast(x, degrees)?, mul(vec![q(-1), from_ast(y, degrees)?])]),
        Ast::Mul(x, y, _) => mul(vec![from_ast(x, degrees)?, from_ast(y, degrees)?]),
        Ast::Div(x, y) => {
            let d = from_ast(y, degrees)?;
            if d.is_zero() {
                return None;
            }
            mul(vec![from_ast(x, degrees)?, pow(d, q(-1))])
        }
        Ast::Pow(x, y) => {
            let b = from_ast(x, degrees)?;
            let e = from_ast(y, degrees)?;
            if b.is_zero() && matches!(&e, E::Q(r) if !r.is_positive()) {
                return None;
            }
            pow(b, e)
        }
        Ast::Deg(x) => {
            let v = from_ast(x, degrees)?;
            if degrees {
                v
            } else {
                mul(vec![v, E::Pi, qr(1, 180)])
            }
        }
        Ast::Call(f, args) if args.len() == 1 => {
            let x = from_ast(&args[0], degrees)?;
            match f.as_str() {
                "sqrt" => {
                    if matches!(&x, E::Q(r) if r.is_negative()) {
                        return None;
                    }
                    pow(x, qr(1, 2))
                }
                "cbrt" => pow(x, qr(1, 3)),
                "abs" => match &x {
                    E::Q(r) => E::Q(r.abs()),
                    _ if !x.has_sym() => {
                        if x.to_f64() < 0.0 {
                            mul(vec![q(-1), x])
                        } else {
                            x
                        }
                    }
                    _ => E::Fn("abs".into(), vec![x]),
                },
                "sin" | "cos" | "tan" => {
                    let arg = if degrees { mul(vec![x, E::Pi, qr(1, 180)]) } else { x };
                    trig_exact(f, &arg)?
                }
                "ln" => match &x {
                    E::Q(r) if r.is_one() => q(0),
                    E::Euler => q(1),
                    E::Pow(b, e) if **b == E::Euler => (**e).clone(),
                    _ => return None,
                },
                _ => return None,
            }
        }
        Ast::Call(f, args) if args.len() == 2 && f == "root" => {
            let x = from_ast(&args[0], degrees)?;
            let n = from_ast(&args[1], degrees)?;
            let n = n.is_q()?.clone();
            pow(x, E::Q(n.recip()))
        }
        _ => return None,
    })
}

/// sin/cos/tan at rational multiples of π with a known closed form.
fn trig_exact(f: &str, arg: &E) -> Option<E> {
    if arg.is_zero() {
        return Some(if f == "cos" { q(1) } else { q(0) });
    }
    // arg must be k·π
    let k = match arg {
        E::Pi => BigRational::one(),
        E::Mul(v) if v.len() == 2 && v[1] == E::Pi => v[0].is_q()?.clone(),
        _ => return None,
    };
    // reduce to [0, 2) in units of π
    let two = BigRational::from_integer(BigInt::from(2));
    let mut t = k.clone() - (k.clone() / two.clone()).floor() * two.clone();
    let mut sign = 1;
    let fname = f;
    let (s, c) = {
        // sin and cos of t·π with quadrant symmetry
        let half = BigRational::new(BigInt::one(), BigInt::from(2));
        let one = BigRational::one();
        let mut ssign = 1;
        let mut csign = 1;
        if t >= one {
            t -= one.clone();
            ssign = -ssign;
            csign = -csign;
        }
        let mut swap = false;
        if t > half {
            t = one.clone() - t;
            csign = -csign;
        }
        if t > BigRational::new(BigInt::one(), BigInt::from(4)) {
            t = half - t;
            swap = true;
        }
        let (sv, cv) = base_trig(&t)?;
        let (sv, cv) = if swap { (cv, sv) } else { (sv, cv) };
        (mul(vec![q(ssign), sv]), mul(vec![q(csign), cv]))
    };
    let _ = &mut sign;
    Some(match fname {
        "sin" => s,
        "cos" => c,
        _ => {
            if c.is_zero() {
                return None; // tan undefined
            }
            simplify(&E::Mul(vec![s, E::Pow(Box::new(c), Box::new(q(-1)))]))
        }
    })
}

/// sin and cos of t·π for t in [0, 1/4].
fn base_trig(t: &BigRational) -> Option<(E, E)> {
    let n = t.numer().to_i64()?;
    let d = t.denom().to_i64()?;
    let s2 = pow(q(2), qr(1, 2));
    let s3 = pow(q(3), qr(1, 2));
    let s6 = pow(q(6), qr(1, 2));
    let s5 = pow(q(5), qr(1, 2));
    Some(match (n, d) {
        (0, _) => (q(0), q(1)),
        (1, 4) => (mul(vec![qr(1, 2), s2.clone()]), mul(vec![qr(1, 2), s2])),
        (1, 6) => (qr(1, 2), mul(vec![qr(1, 2), s3])),
        (1, 12) => (
            mul(vec![qr(1, 4), add(vec![s6.clone(), mul(vec![q(-1), s2.clone()])])]),
            mul(vec![qr(1, 4), add(vec![s6, s2])]),
        ),
        (1, 10) => (
            mul(vec![qr(1, 4), add(vec![s5.clone(), q(-1)])]),
            pow(mul(vec![qr(1, 8), add(vec![q(5), s5])]), qr(1, 2)),
        ),
        (1, 5) => (
            pow(mul(vec![qr(1, 8), add(vec![q(5), mul(vec![q(-1), s5.clone()])])]), qr(1, 2)),
            mul(vec![qr(1, 4), add(vec![s5, q(1)])]),
        ),
        (1, 8) => (
            mul(vec![qr(1, 2), pow(add(vec![q(2), mul(vec![q(-1), s2.clone()])]), qr(1, 2))]),
            mul(vec![qr(1, 2), pow(add(vec![q(2), s2]), qr(1, 2))]),
        ),
        _ => return None,
    })
}

// ------------------------------------------------------------------ canonical ordering

fn rank(e: &E) -> u8 {
    match e {
        E::Q(_) => 0,
        E::Pi | E::Euler => 1,
        E::Pow(b, _) if b.is_q().is_some() => 2, // radicals after constants
        E::Sym(_) => 3,
        E::Pow(..) => 4,
        E::Mul(_) => 5,
        E::Fn(..) => 6,
        E::Add(_) => 7,
    }
}

pub fn cmp(a: &E, b: &E) -> Ordering {
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (E::Q(x), E::Q(y)) => x.cmp(y),
        (E::Sym(x), E::Sym(y)) => x.cmp(y),
        (E::Pi, E::Euler) => Ordering::Less,
        (E::Euler, E::Pi) => Ordering::Greater,
        (E::Pow(a1, b1), E::Pow(a2, b2)) => cmp(a1, a2).then_with(|| cmp(b1, b2)),
        (E::Fn(f, x), E::Fn(g, y)) => f.cmp(g).then_with(|| cmp_list(x, y)),
        (E::Add(x), E::Add(y)) | (E::Mul(x), E::Mul(y)) => cmp_list(x, y),
        _ => Ordering::Equal,
    }
}
fn cmp_list(x: &[E], y: &[E]) -> Ordering {
    for (a, b) in x.iter().zip(y) {
        let c = cmp(a, b);
        if c != Ordering::Equal {
            return c;
        }
    }
    x.len().cmp(&y.len())
}

// ------------------------------------------------------------------ simplification

/// Split a product into (rational coefficient, the rest as a sorted factor list).
fn coeff_rest(e: &E) -> (BigRational, Vec<E>) {
    match e {
        E::Q(r) => (r.clone(), vec![]),
        E::Mul(v) => {
            let mut c = BigRational::one();
            let mut rest = vec![];
            for f in v {
                match f {
                    E::Q(r) => c *= r,
                    o => rest.push(o.clone()),
                }
            }
            (c, rest)
        }
        o => (BigRational::one(), vec![o.clone()]),
    }
}

fn from_factors(c: BigRational, mut rest: Vec<E>) -> E {
    if c.is_zero() {
        return E::Q(c);
    }
    if rest.is_empty() {
        return E::Q(c);
    }
    if !c.is_one() {
        rest.insert(0, E::Q(c));
    }
    if rest.len() == 1 {
        rest.pop().unwrap()
    } else {
        E::Mul(rest)
    }
}

pub fn simplify(e: &E) -> E {
    match e {
        E::Add(v) => {
            let mut terms: Vec<E> = Vec::new();
            for t in v {
                match simplify(t) {
                    E::Add(inner) => terms.extend(inner),
                    o => terms.push(o),
                }
            }
            // collect like terms: coefficient · same factor list
            let mut acc: Vec<(BigRational, Vec<E>)> = Vec::new();
            for t in terms {
                let (c, rest) = coeff_rest(&t);
                if let Some(slot) = acc.iter_mut().find(|(_, r)| *r == rest) {
                    slot.0 += c;
                } else {
                    acc.push((c, rest));
                }
            }
            let mut out: Vec<E> = acc.into_iter().filter(|(c, _)| !c.is_zero()).map(|(c, r)| from_factors(c, r)).collect();
            out.sort_by(|a, b| cmp_terms(a, b));
            match out.len() {
                0 => q(0),
                1 => out.pop().unwrap(),
                _ => E::Add(out),
            }
        }
        E::Mul(v) => {
            let mut fs: Vec<E> = Vec::new();
            for f in v {
                match simplify(f) {
                    E::Mul(inner) => fs.extend(inner),
                    o => fs.push(o),
                }
            }
            let mut c = BigRational::one();
            // base → exponent
            let mut powers: Vec<(E, E)> = Vec::new();
            for f in fs {
                match f {
                    E::Q(r) => c *= r,
                    E::Pow(b, x) => push_power(&mut powers, *b, *x),
                    o => push_power(&mut powers, o, q(1)),
                }
            }
            if c.is_zero() {
                return q(0);
            }
            let mut rest: Vec<E> = Vec::new();
            for (b, x) in powers {
                match pow(b, x) {
                    E::Q(r) => c *= r,
                    E::Mul(inner) => {
                        for f in inner {
                            match f {
                                E::Q(r) => c *= r,
                                o => rest.push(o),
                            }
                        }
                    }
                    o => rest.push(o),
                }
            }
            // merge radicals of rationals with the same index: √2·√3 = √6
            rest = merge_radicals(rest, &mut c);
            rest.sort_by(cmp);
            // distribute a rational over a sum? no: keep 2(x+1) as is
            from_factors(c, rest)
        }
        E::Pow(b, x) => {
            let b = simplify(b);
            let x = simplify(x);
            if x.is_zero() {
                return q(1);
            }
            if x.is_one() {
                return b;
            }
            if b.is_one() {
                return q(1);
            }
            match (&b, &x) {
                (E::Q(br), E::Q(xr)) => rat_pow(br, xr),
                // (a^m)^n = a^(mn) for rational exponents when safe (a ≥ 0 or n integer)
                (E::Pow(bb, m), E::Q(n)) if n.is_integer() || matches!(&**bb, E::Q(r) if !r.is_negative()) || matches!(**bb, E::Pi | E::Euler) => {
                    pow((**bb).clone(), mul(vec![(**m).clone(), x.clone()]))
                }
                // (ab)^n = a^n b^n for integer n
                (E::Mul(fs), E::Q(n)) if n.is_integer() => mul(fs.iter().map(|f| pow(f.clone(), x.clone())).collect()),
                _ => E::Pow(Box::new(b), Box::new(x)),
            }
        }
        E::Fn(f, a) => E::Fn(f.clone(), a.iter().map(simplify).collect()),
        o => o.clone(),
    }
}

fn cmp_terms(a: &E, b: &E) -> Ordering {
    // polynomial-style order: higher degree first, constants last
    let da = degree(a);
    let db = degree(b);
    db.partial_cmp(&da).unwrap_or(Ordering::Equal).then_with(|| {
        let (_, ra) = coeff_rest(a);
        let (_, rb) = coeff_rest(b);
        cmp_list(&ra, &rb)
    })
}
fn degree(e: &E) -> f64 {
    match e {
        E::Sym(_) => 1.0,
        E::Pow(b, x) if b.has_sym() => x.to_f64() * degree(b),
        E::Mul(v) => v.iter().map(degree).sum(),
        E::Q(_) => 0.0,
        E::Pi | E::Euler => 0.0,
        E::Pow(..) => 0.0,
        _ => 0.5,
    }
}

fn push_power(p: &mut Vec<(E, E)>, b: E, x: E) {
    if let Some(slot) = p.iter_mut().find(|(bb, _)| *bb == b) {
        slot.1 = add(vec![slot.1.clone(), x]);
    } else {
        p.push((b, x));
    }
}

/// √2·√3 → √6 (same root index, positive rational bases).
fn merge_radicals(rest: Vec<E>, c: &mut BigRational) -> Vec<E> {
    let mut out: Vec<E> = Vec::new();
    let mut by_index: Vec<(BigInt, BigRational)> = Vec::new(); // denominator of exponent 1/n → product of bases
    for f in rest {
        if let E::Pow(b, x) = &f {
            if let (E::Q(br), E::Q(xr)) = (&**b, &**x) {
                if br.is_positive() && xr.numer().is_one() {
                    let n = xr.denom().clone();
                    if let Some(slot) = by_index.iter_mut().find(|(k, _)| *k == n) {
                        slot.1 *= br;
                    } else {
                        by_index.push((n, br.clone()));
                    }
                    continue;
                }
            }
        }
        out.push(f);
    }
    for (n, base) in by_index {
        match rat_pow(&base, &BigRational::new(BigInt::one(), n)) {
            E::Q(r) => *c *= r,
            E::Mul(v) => {
                for f in v {
                    match f {
                        E::Q(r) => *c *= r,
                        o => out.push(o),
                    }
                }
            }
            o => out.push(o),
        }
    }
    out
}

/// a^(p/q) for rationals: integer powers exactly, roots with perfect powers pulled out (√12 = 2√3),
/// denominators rationalised (1/√2 = √2/2), odd roots of negatives real (∛-8 = -2).
fn rat_pow(b: &BigRational, x: &BigRational) -> E {
    if x.is_integer() {
        let n = x.to_integer();
        if let Some(k) = n.abs().to_u32().filter(|k| *k <= 4096) {
            if b.is_zero() && n.is_negative() {
                return E::Pow(Box::new(E::Q(b.clone())), Box::new(E::Q(x.clone())));
            }
            let bits = b.numer().bits().max(b.denom().bits()) * k as u64;
            if bits < 200_000 {
                let p = num_traits::pow(b.clone(), k as usize);
                return E::Q(if n.is_negative() { p.recip() } else { p });
            }
        }
        return E::Pow(Box::new(E::Q(b.clone())), Box::new(E::Q(x.clone())));
    }
    let Some(qd) = x.denom().to_u32().filter(|d| *d <= 64) else {
        return E::Pow(Box::new(E::Q(b.clone())), Box::new(E::Q(x.clone())));
    };
    let p = x.numer().clone();
    // integer part of the exponent: b^(p/q) = b^k · b^(r/q)
    let (k, r) = p.div_mod_floor(&BigInt::from(qd));
    let mut coeff = BigRational::one();
    if !k.is_zero() {
        if let E::Q(v) = rat_pow(b, &BigRational::from_integer(k)) {
            coeff = v;
        }
    }
    if r.is_zero() {
        return E::Q(coeff);
    }
    let neg = b.is_negative();
    if neg && qd % 2 == 0 {
        return E::Pow(Box::new(E::Q(b.clone())), Box::new(E::Q(x.clone())));
    }
    let r = r.to_u32().unwrap_or(1);
    // rationalise: (n/d)^(r/q) = (n·d^(q-1))^(r/q) / d^r
    let n = b.numer().abs();
    let d = b.denom().clone();
    let inside = n * num_traits::pow(d.clone(), (qd - 1) as usize);
    let outside_den = num_traits::pow(d, r as usize);
    // inside^(r/q): pull out perfect q-th powers of inside^r
    let radicand = num_traits::pow(inside, r as usize);
    let (out, rem) = extract_power(&radicand, qd);
    coeff *= BigRational::new(out, outside_den);
    if neg && (r % 2 == 1) {
        coeff = -coeff;
    }
    // reduce the root index when possible: ⁴√9 = √3
    let (rem, idx) = reduce_index(rem, qd);
    if rem.is_one() {
        return E::Q(coeff);
    }
    let rad = E::Pow(Box::new(E::Q(BigRational::from_integer(rem))), Box::new(E::Q(BigRational::new(BigInt::one(), BigInt::from(idx)))));
    if coeff.is_one() {
        rad
    } else {
        E::Mul(vec![E::Q(coeff), rad])
    }
}

/// n = out^q · rem with rem q-th-power-free (trial division; fine for student-size numbers).
fn extract_power(n: &BigInt, qd: u32) -> (BigInt, BigInt) {
    let mut out = BigInt::one();
    let mut rem = BigInt::one();
    let mut m = n.clone();
    if m.bits() > 128 {
        return (BigInt::one(), m);
    }
    let mut p = BigInt::from(2);
    while &p * &p <= m && p < BigInt::from(1_000_000) {
        let mut e = 0u32;
        while (&m % &p).is_zero() {
            m /= &p;
            e += 1;
        }
        if e > 0 {
            out *= num_traits::pow(p.clone(), (e / qd) as usize);
            rem *= num_traits::pow(p.clone(), (e % qd) as usize);
        }
        p += 1;
    }
    rem *= m;
    (out, rem)
}

fn reduce_index(rem: BigInt, idx: u32) -> (BigInt, u32) {
    for d in (2..=idx).rev() {
        if idx % d == 0 {
            if let Some(r) = crate::num::iroot(&rem, d) {
                return reduce_index(r, idx / d);
            }
        }
    }
    (rem, idx)
}

// ------------------------------------------------------------------ text

pub fn plain(e: &E) -> String {
    match e {
        E::Q(r) if r.is_integer() => r.to_integer().to_string(),
        E::Q(r) => format!("{}/{}", r.numer(), r.denom()),
        E::Sym(s) => s.clone(),
        E::Pi => "π".into(),
        E::Euler => "e".into(),
        E::Add(v) => {
            let mut s = String::new();
            for (i, t) in v.iter().enumerate() {
                let p = plain(t);
                if i > 0 {
                    if let Some(rest) = p.strip_prefix('-') {
                        s.push_str(" − ");
                        s.push_str(rest);
                        continue;
                    }
                    s.push_str(" + ");
                }
                s.push_str(&p);
            }
            s
        }
        E::Mul(v) => {
            let (c, rest) = coeff_rest(e);
            let num: Vec<String> = rest.iter().filter(|f| !is_recip(f)).map(|f| paren_mul(f)).collect();
            let den: Vec<String> = rest.iter().filter_map(recip_of).map(|f| paren_mul(&f)).collect();
            let mut s = String::new();
            let cn = c.numer().clone();
            let cd = c.denom().clone();
            if cn == -BigInt::one() {
                s.push('-');
            } else if !cn.is_one() || num.is_empty() {
                s.push_str(&cn.to_string());
            }
            s.push_str(&num.join("·"));
            let mut dens = Vec::new();
            if !cd.is_one() {
                dens.push(cd.to_string());
            }
            dens.extend(den);
            if !dens.is_empty() {
                s.push('/');
                if dens.len() > 1 {
                    s.push_str(&format!("({})", dens.join("·")));
                } else {
                    s.push_str(&dens[0]);
                }
            }
            let _ = v;
            s
        }
        E::Pow(b, x) => match &**x {
            E::Q(r) if r.numer().is_one() && r.denom() == &BigInt::from(2) => format!("√{}", paren_root(b)),
            E::Q(r) if r.numer().is_one() && r.denom() == &BigInt::from(3) => format!("∛{}", paren_root(b)),
            E::Q(r) if r.numer().is_one() => format!("root({}, {})", plain(b), r.denom()),
            _ => format!("{}^{}", paren_pow(b), paren_pow(x)),
        },
        E::Fn(f, a) => format!("{f}({})", a.iter().map(plain).collect::<Vec<_>>().join(", ")),
    }
}
fn is_recip(e: &E) -> bool {
    matches!(e, E::Pow(_, x) if matches!(&**x, E::Q(r) if r.is_negative()))
}
fn recip_of(e: &E) -> Option<E> {
    if let E::Pow(b, x) = e {
        if let E::Q(r) = &**x {
            if r.is_negative() {
                return Some(pow((**b).clone(), E::Q(-r.clone())));
            }
        }
    }
    None
}
fn paren_mul(e: &E) -> String {
    match e {
        E::Add(_) => format!("({})", plain(e)),
        _ => plain(e),
    }
}
fn paren_pow(e: &E) -> String {
    match e {
        E::Sym(_) | E::Pi | E::Euler => plain(e),
        E::Q(r) if r.is_integer() && !r.is_negative() => plain(e),
        _ => format!("({})", plain(e)),
    }
}
fn paren_root(e: &E) -> String {
    match e {
        E::Q(r) if r.is_integer() => plain(e),
        E::Sym(_) => plain(e),
        _ => format!("({})", plain(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::collections::HashSet;

    fn s(src: &str) -> String {
        let a = parse(src, &HashSet::new()).unwrap().unwrap();
        plain(&from_ast(&a, false).unwrap())
    }

    #[test]
    fn radicals() {
        assert_eq!(s("sqrt(8)"), "2√2");
        assert_eq!(s("sqrt(12)"), "2√3");
        assert_eq!(s("sqrt(2)*sqrt(3)"), "√6");
        assert_eq!(s("1/sqrt(2)"), "√2/2");
        assert_eq!(s("cbrt(54)"), "3∛2");
        assert_eq!(s("sqrt(18)+sqrt(8)"), "5√2");
        assert_eq!(s("sqrt(3/4)"), "√3/2");
        assert_eq!(s("4^(3/2)"), "8");
    }

    #[test]
    fn constants_and_terms() {
        assert_eq!(s("pi/2 + pi/3"), "5π/6");
        assert_eq!(s("2x + 3x - y"), "5x − y");
        assert_eq!(s("x*x^2"), "x^3");
        assert_eq!(s("sin(pi/6)"), "1/2");
        assert_eq!(s("cos(pi/4)"), "√2/2");
        assert_eq!(s("tan(pi/3)"), "√3");
        assert_eq!(s("sin(3pi/4)"), "√2/2");
        assert_eq!(s("cos(2pi/3)"), "-1/2");
        assert_eq!(s("ln(e^2)"), "2");
    }
}
