//! Evaluation of syntax trees to values: exact while possible, lists broadcast like in Desmos,
//! matrices with exact rational entries (det, inverse, rank, rref, powers, transpose).

use crate::num::{fmt_float, Num};
use crate::parse::{Ast, Rel};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Val {
    N(Num),
    List(Vec<Val>),
    Mat(Mat),
    Tuple(Vec<Val>),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub struct Mat {
    pub r: usize,
    pub c: usize,
    pub d: Vec<Num>,
}

impl Mat {
    pub fn at(&self, i: usize, j: usize) -> &Num {
        &self.d[i * self.c + j]
    }
    pub fn eye(n: usize) -> Mat {
        let mut d = vec![Num::zero(); n * n];
        for i in 0..n {
            d[i * n + i] = Num::one();
        }
        Mat { r: n, c: n, d }
    }
    pub fn transpose(&self) -> Mat {
        let mut d = Vec::with_capacity(self.d.len());
        for j in 0..self.c {
            for i in 0..self.r {
                d.push(self.at(i, j).clone());
            }
        }
        Mat { r: self.c, c: self.r, d }
    }
    pub fn mul(&self, o: &Mat) -> Result<Mat, String> {
        if self.c != o.r {
            return Err(format!("can't multiply a {}×{} matrix by a {}×{} one (columns ≠ rows)", self.r, self.c, o.r, o.c));
        }
        let mut d = Vec::with_capacity(self.r * o.c);
        for i in 0..self.r {
            for j in 0..o.c {
                let mut s = Num::zero();
                for k in 0..self.c {
                    s = s.add(&self.at(i, k).mul(o.at(k, j))?)?;
                }
                d.push(s);
            }
        }
        Ok(Mat { r: self.r, c: o.c, d })
    }
    /// Reduced row echelon form (exact) and the rank; the sign/scale factor of det comes along.
    pub fn rref(&self) -> Result<(Mat, usize, Num), String> {
        let mut m = self.clone();
        let mut row = 0;
        let mut det = Num::one();
        for col in 0..m.c {
            if row >= m.r {
                break;
            }
            let Some(p) = (row..m.r).find(|&i| !m.at(i, col).is_zero()) else {
                det = Num::zero();
                continue;
            };
            if p != row {
                for j in 0..m.c {
                    m.d.swap(row * m.c + j, p * m.c + j);
                }
                det = det.neg();
            }
            let pv = m.at(row, col).clone();
            det = det.mul(&pv)?;
            for j in 0..m.c {
                let v = m.at(row, j).div(&pv)?;
                m.d[row * m.c + j] = v;
            }
            for i in 0..m.r {
                if i != row && !m.at(i, col).is_zero() {
                    let f = m.at(i, col).clone();
                    for j in 0..m.c {
                        let v = m.at(i, j).sub(&f.mul(m.at(row, j))?)?;
                        m.d[i * m.c + j] = v;
                    }
                }
            }
            row += 1;
        }
        if row < m.r.min(m.c) {
            det = Num::zero();
        }
        Ok((m, row, det))
    }
    pub fn det(&self) -> Result<Num, String> {
        if self.r != self.c {
            return Err(format!("det needs a square matrix (this one is {}×{})", self.r, self.c));
        }
        Ok(self.rref()?.2)
    }
    pub fn inv(&self) -> Result<Mat, String> {
        if self.r != self.c {
            return Err(format!("only square matrices have an inverse (this one is {}×{})", self.r, self.c));
        }
        let n = self.r;
        let mut aug = Mat { r: n, c: 2 * n, d: Vec::with_capacity(2 * n * n) };
        for i in 0..n {
            for j in 0..n {
                aug.d.push(self.at(i, j).clone());
            }
            for j in 0..n {
                aug.d.push(if i == j { Num::one() } else { Num::zero() });
            }
        }
        let (r, _, _) = aug.rref()?;
        for i in 0..n {
            if !r.at(i, i).is_zero() && r.at(i, i).sub(&Num::one())?.is_zero() {
                continue;
            }
            return Err("the matrix is singular (det = 0), so it has no inverse".into());
        }
        let mut d = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in n..2 * n {
                d.push(r.at(i, j).clone());
            }
        }
        Ok(Mat { r: n, c: n, d })
    }
    fn zip(&self, o: &Mat, f: impl Fn(&Num, &Num) -> Result<Num, String>, what: &str) -> Result<Mat, String> {
        if self.r != o.r || self.c != o.c {
            return Err(format!("can't {what} a {}×{} and a {}×{} matrix", self.r, self.c, o.r, o.c));
        }
        let d = self.d.iter().zip(&o.d).map(|(a, b)| f(a, b)).collect::<Result<_, _>>()?;
        Ok(Mat { r: self.r, c: self.c, d })
    }
    fn map(&self, f: impl Fn(&Num) -> Result<Num, String>) -> Result<Mat, String> {
        Ok(Mat { r: self.r, c: self.c, d: self.d.iter().map(f).collect::<Result<_, _>>()? })
    }
}

#[derive(Debug)]
pub enum EvalErr {
    Msg(String),
    Free(String),
}
impl From<String> for EvalErr {
    fn from(s: String) -> Self {
        EvalErr::Msg(s)
    }
}
impl From<&str> for EvalErr {
    fn from(s: &str) -> Self {
        EvalErr::Msg(s.into())
    }
}
pub type R = Result<Val, EvalErr>;

#[derive(Default)]
pub struct Env {
    pub vars: HashMap<String, Val>,
    pub funcs: HashMap<String, (Vec<String>, Ast)>,
    pub degrees: bool,
    pub depth: usize,
}

impl Env {
    pub fn eval(&mut self, a: &Ast, loc: &HashMap<String, Val>) -> R {
        self.depth += 1;
        if self.depth > 200 {
            self.depth -= 1;
            return Err("too deeply nested (a definition refers to itself?)".into());
        }
        let r = self.eval_inner(a, loc);
        self.depth -= 1;
        r
    }

    fn eval_inner(&mut self, a: &Ast, loc: &HashMap<String, Val>) -> R {
        Ok(match a {
            Ast::Num(s) => Val::N(Num::parse_decimal(s).ok_or_else(|| format!("bad number {s}"))?),
            Ast::Var(n) => {
                if let Some(v) = loc.get(n).or_else(|| self.vars.get(n)) {
                    v.clone()
                } else {
                    match n.as_str() {
                        "pi" => Val::N(Num::R(std::f64::consts::PI)),
                        "e" => Val::N(Num::R(std::f64::consts::E)),
                        "∞" => Val::N(Num::R(f64::INFINITY)),
                        "tau" => Val::N(Num::R(std::f64::consts::TAU)),
                        _ => return Err(EvalErr::Free(n.clone())),
                    }
                }
            }
            Ast::Neg(x) => {
                let v = self.eval(x, loc)?;
                map1(&v, &|n| Ok(n.neg()))?
            }
            Ast::Add(x, y) => arith(&self.eval(x, loc)?, &self.eval(y, loc)?, '+')?,
            Ast::Sub(x, y) => arith(&self.eval(x, loc)?, &self.eval(y, loc)?, '-')?,
            Ast::Mul(x, y, _) => arith(&self.eval(x, loc)?, &self.eval(y, loc)?, '*')?,
            Ast::Div(x, y) => arith(&self.eval(x, loc)?, &self.eval(y, loc)?, '/')?,
            Ast::Pow(x, y) => {
                let b = self.eval(x, loc)?;
                if let (Val::Mat(m), Ast::Var(t)) = (&b, &**y) {
                    if t == "T" && !self.vars.contains_key("T") && !loc.contains_key("T") {
                        return Ok(Val::Mat(m.transpose()));
                    }
                }
                arith(&b, &self.eval(y, loc)?, '^')?
            }
            Ast::Fact(x) => map1(&self.eval(x, loc)?, &factorial)?,
            Ast::Deg(x) => {
                let v = self.eval(x, loc)?;
                if self.degrees {
                    v
                } else {
                    map1(&v, &|n| n.mul(&Num::R(std::f64::consts::PI / 180.0)))?
                }
            }
            Ast::Abs(x) => match self.eval(x, loc)? {
                Val::Mat(m) => Val::N(m.det()?),
                v => map1(&v, &|n| Ok(n.abs()))?,
            },
            Ast::List(items) => Val::List(items.iter().map(|i| self.eval(i, loc)).collect::<Result<_, _>>()?),
            Ast::Tuple(items) => Val::Tuple(items.iter().map(|i| self.eval(i, loc)).collect::<Result<_, _>>()?),
            Ast::Matrix(rows) => {
                let (r, c) = (rows.len(), rows[0].len());
                let mut d = Vec::with_capacity(r * c);
                for row in rows {
                    for e in row {
                        match self.eval(e, loc)? {
                            Val::N(n) => d.push(n),
                            _ => return Err("matrix entries must be numbers".into()),
                        }
                    }
                }
                Val::Mat(Mat { r, c, d })
            }
            Ast::Rel(first, chain) => {
                let mut l = self.eval(first, loc)?;
                let mut ok = true;
                for (rel, e) in chain {
                    let r = self.eval(e, loc)?;
                    ok &= compare(&l, &r, *rel)?;
                    l = r;
                }
                Val::Bool(ok)
            }
            Ast::Piecewise(cases, other) => {
                for (c, v) in cases {
                    if let Val::Bool(true) = self.eval(c, loc)? {
                        return self.eval(v, loc);
                    }
                }
                match other {
                    Some(o) => self.eval(o, loc)?,
                    None => return Err("undefined (no case applies)".into()),
                }
            }
            Ast::Restrict(e, conds) => {
                for c in conds {
                    if let Val::Bool(false) = self.eval(c, loc)? {
                        return Err("undefined (outside the restriction)".into());
                    }
                }
                self.eval(e, loc)?
            }
            Ast::Call(f, args) => self.call(f, args, loc)?,
        })
    }

    fn call(&mut self, f: &str, args: &[Ast], loc: &HashMap<String, Val>) -> R {
        if let Some((params, body)) = self.funcs.get(f).cloned() {
            if params.len() != args.len() {
                return Err(format!("{f} takes {} argument(s), got {}", params.len(), args.len()).into());
            }
            let mut l2 = loc.clone();
            for (p, a) in params.iter().zip(args) {
                let v = self.eval(a, loc)?;
                l2.insert(p.clone(), v);
            }
            return self.eval(&body, &l2);
        }
        let vals: Vec<Val> = args.iter().map(|a| self.eval(a, loc)).collect::<Result<_, _>>()?;
        builtin(f, &vals, self.degrees)
    }
}

fn arith(a: &Val, b: &Val, op: char) -> R {
    let numop = |x: &Num, y: &Num| -> Result<Num, String> {
        match op {
            '+' => x.add(y),
            '-' => x.sub(y),
            '*' => x.mul(y),
            '/' => x.div(y),
            _ => x.pow(y),
        }
    };
    Ok(match (a, b) {
        (Val::N(x), Val::N(y)) => Val::N(numop(x, y)?),
        (Val::Mat(m), Val::Mat(n)) => match op {
            '+' => Val::Mat(m.zip(n, |x, y| x.add(y), "add")?),
            '-' => Val::Mat(m.zip(n, |x, y| x.sub(y), "subtract")?),
            '*' => Val::Mat(m.mul(n)?),
            '/' => Val::Mat(m.mul(&n.inv()?)?),
            _ => return Err("a matrix power needs a whole-number exponent".into()),
        },
        (Val::Mat(m), Val::N(k)) if op == '^' => Val::Mat(mat_pow(m, k)?),
        (Val::Mat(m), Val::N(k)) if op == '*' || op == '/' => Val::Mat(m.map(|x| numop(x, k))?),
        (Val::N(k), Val::Mat(m)) if op == '*' => Val::Mat(m.map(|x| k.mul(x))?),
        (Val::Mat(_), _) | (_, Val::Mat(_)) => return Err(format!("can't use '{op}' between a matrix and that").into()),
        (Val::List(x), Val::List(y)) => {
            Val::List(x.iter().zip(y).map(|(p, q)| arith(p, q, op)).collect::<Result<_, _>>()?)
        }
        (Val::List(x), y) => Val::List(x.iter().map(|p| arith(p, y, op)).collect::<Result<_, _>>()?),
        (x, Val::List(y)) => Val::List(y.iter().map(|q| arith(x, q, op)).collect::<Result<_, _>>()?),
        (Val::Tuple(x), Val::Tuple(y)) if (op == '+' || op == '-') && x.len() == y.len() => {
            Val::Tuple(x.iter().zip(y).map(|(p, q)| arith(p, q, op)).collect::<Result<_, _>>()?)
        }
        (Val::Tuple(x), Val::N(_)) if op == '*' || op == '/' => {
            Val::Tuple(x.iter().map(|p| arith(p, b, op)).collect::<Result<_, _>>()?)
        }
        (Val::N(_), Val::Tuple(y)) if op == '*' => Val::Tuple(y.iter().map(|q| arith(a, q, op)).collect::<Result<_, _>>()?),
        _ => return Err(format!("can't use '{op}' on these values").into()),
    })
}

fn mat_pow(m: &Mat, k: &Num) -> Result<Mat, String> {
    if m.r != m.c {
        return Err("only square matrices have powers".into());
    }
    let n = k.as_i64().ok_or("a matrix power needs a whole-number exponent")?;
    let mut base = if n < 0 { m.inv()? } else { m.clone() };
    let mut e = n.unsigned_abs();
    if e > 100_000 {
        return Err("exponent too large".into());
    }
    let mut acc = Mat::eye(m.r);
    while e > 0 {
        if e & 1 == 1 {
            acc = acc.mul(&base)?;
        }
        e >>= 1;
        if e > 0 {
            base = base.mul(&base)?;
        }
    }
    Ok(acc)
}

fn map1(v: &Val, f: &dyn Fn(&Num) -> Result<Num, String>) -> R {
    Ok(match v {
        Val::N(n) => Val::N(f(n)?),
        Val::List(l) => Val::List(l.iter().map(|x| map1(x, f)).collect::<Result<_, _>>()?),
        Val::Mat(m) => Val::Mat(m.map(f)?),
        Val::Tuple(l) => Val::Tuple(l.iter().map(|x| map1(x, f)).collect::<Result<_, _>>()?),
        Val::Bool(_) => return Err("that needs a number, not true/false".into()),
    })
}

fn compare(a: &Val, b: &Val, rel: Rel) -> Result<bool, EvalErr> {
    let (Val::N(x), Val::N(y)) = (a, b) else {
        return Err("only numbers can be compared".into());
    };
    let d = x.sub(y)?;
    let s = match &d {
        Num::R(v) if v.abs() <= 1e-12 * (1.0 + x.to_f64().abs()) => 0,
        d => d.sign(),
    };
    Ok(match rel {
        Rel::Eq => s == 0,
        Rel::Ne => s != 0,
        Rel::Lt => s < 0,
        Rel::Le => s <= 0,
        Rel::Gt => s > 0,
        Rel::Ge => s >= 0,
    })
}

fn factorial(n: &Num) -> Result<Num, String> {
    let k = n.as_i64().filter(|k| *k >= 0).ok_or("factorial needs a whole number ≥ 0")?;
    if k > 3000 {
        return Err("too large for an exact factorial".into());
    }
    let mut p = BigInt::from(1);
    for i in 2..=k {
        p *= i;
    }
    Ok(Num::Q(BigRational::from_integer(p)))
}

fn int_arg(v: &Val, what: &str) -> Result<BigInt, EvalErr> {
    match v {
        Val::N(Num::Q(q)) if q.is_integer() => Ok(q.to_integer()),
        Val::N(n) if n.is_integer() => Ok(BigInt::from(n.as_i64().unwrap_or(0))),
        _ => Err(format!("{what} needs whole numbers").into()),
    }
}

fn nums(vals: &[Val]) -> Vec<Num> {
    let mut out = Vec::new();
    for v in vals {
        match v {
            Val::N(n) => out.push(n.clone()),
            Val::List(l) => out.extend(nums(l)),
            _ => {}
        }
    }
    out
}

fn builtin(f: &str, v: &[Val], deg: bool) -> R {
    let one = |what: &str| -> Result<&Val, EvalErr> {
        if v.len() == 1 {
            Ok(&v[0])
        } else {
            Err(format!("{what} takes one argument").into())
        }
    };
    let mat = |what: &str| -> Result<&Mat, EvalErr> {
        match one(what)? {
            Val::Mat(m) => Ok(m),
            _ => Err(format!("{what} needs a matrix").into()),
        }
    };
    let ang = |x: f64| if deg { x.to_radians() } else { x };
    let unang = |x: f64| if deg { x.to_degrees() } else { x };
    let real = |g: &dyn Fn(f64) -> f64| -> R {
        map1(one(f)?, &|n: &Num| {
            let y = g(n.to_f64());
            if y.is_nan() {
                Err(format!("undefined ({f} of {})", n.plain(6)))
            } else {
                Ok(Num::R(y))
            }
        })
    };
    match f {
        "sin" => real(&|x| ang(x).sin()),
        "cos" => real(&|x| ang(x).cos()),
        "tan" => real(&|x| ang(x).tan()),
        "cot" => real(&|x| 1.0 / ang(x).tan()),
        "sec" => real(&|x| 1.0 / ang(x).cos()),
        "csc" => real(&|x| 1.0 / ang(x).sin()),
        "arcsin" => real(&|x| unang(x.asin())),
        "arccos" => real(&|x| unang(x.acos())),
        "arctan" => real(&|x| unang(x.atan())),
        "arccot" => real(&|x| unang(std::f64::consts::FRAC_PI_2 - x.atan())),
        "arcsec" => real(&|x| unang((1.0 / x).acos())),
        "arccsc" => real(&|x| unang((1.0 / x).asin())),
        "sinh" => real(&f64::sinh),
        "cosh" => real(&f64::cosh),
        "tanh" => real(&f64::tanh),
        "coth" => real(&|x| 1.0 / x.tanh()),
        "arcsinh" => real(&f64::asinh),
        "arccosh" => real(&f64::acosh),
        "arctanh" => real(&f64::atanh),
        "exp" => real(&f64::exp),
        "ln" => real(&|x| if x > 0.0 { x.ln() } else { f64::NAN }),
        "lg" => real(&|x| if x > 0.0 { x.log10() } else { f64::NAN }),
        "log" if v.len() == 2 => {
            // log_b x: exact when x is an integer power of b
            if let (Val::N(x), Val::N(b)) = (&v[0], &v[1]) {
                if x.is_exact() && b.is_exact() && b.to_f64() > 0.0 && b.to_f64() != 1.0 {
                    let k = (x.to_f64().ln() / b.to_f64().ln()).round();
                    if k.abs() < 2000.0 {
                        if let Ok(p) = b.pow(&Num::int(k as i64)) {
                            if p.sub(x).map(|d| d.is_zero()).unwrap_or(false) {
                                return Ok(Val::N(Num::int(k as i64)));
                            }
                        }
                    }
                }
                let (x, b) = (x.to_f64(), b.to_f64());
                if x <= 0.0 || b <= 0.0 || b == 1.0 {
                    return Err("undefined (log of a non-positive number or base)".into());
                }
                return Ok(Val::N(Num::R(x.ln() / b.ln())));
            }
            Err("log needs numbers".into())
        }
        "log" => real(&|x| if x > 0.0 { x.log10() } else { f64::NAN }),
        "sqrt" => map1(one(f)?, &|n| n.pow(&Num::ratio(1, 2))),
        "cbrt" => map1(one(f)?, &|n| n.pow(&Num::ratio(1, 3))),
        "root" if v.len() == 2 => match (&v[0], &v[1]) {
            (x, Val::N(k)) => map1(x, &|n| n.pow(&Num::one().div(k)?)),
            _ => Err("root(x, n) needs a number n".into()),
        },
        "abs" => match one(f)? {
            Val::Mat(m) => Ok(Val::N(m.det()?)),
            x => map1(x, &|n| Ok(n.abs())),
        },
        "sign" => map1(one(f)?, &|n| Ok(Num::int(n.sign() as i64))),
        "floor" | "ceil" | "round" | "frac" => map1(one(f)?, &|n| match n {
            Num::Q(q) => Ok(Num::Q(match f {
                "floor" => q.floor(),
                "ceil" => q.ceil(),
                "round" => q.round(),
                _ => q - q.floor(),
            })),
            Num::R(x) => Ok(Num::R(match f {
                "floor" => x.floor(),
                "ceil" => x.ceil(),
                "round" => x.round(),
                _ => x - x.floor(),
            })),
        }),
        "fact" => map1(one(f)?, &factorial),
        "nCr" | "nPr" if v.len() == 2 => {
            let (n, k) = (int_arg(&v[0], f)?, int_arg(&v[1], f)?);
            if k < BigInt::zero() || k > n {
                return Ok(Val::N(Num::zero()));
            }
            let k = k.to_u64().ok_or("too large")?;
            if k > 100_000 {
                return Err("too large".into());
            }
            let mut p = BigInt::from(1);
            for i in 0..k {
                p *= &n - BigInt::from(i);
            }
            if f == "nCr" {
                for i in 1..=k {
                    p /= BigInt::from(i);
                }
            }
            Ok(Val::N(Num::Q(BigRational::from_integer(p))))
        }
        "gcd" | "lcm" => {
            let ns = nums(v);
            if ns.is_empty() {
                return Err(format!("{f} needs numbers").into());
            }
            let mut acc = int_arg(&Val::N(ns[0].clone()), f)?;
            for n in &ns[1..] {
                let b = int_arg(&Val::N(n.clone()), f)?;
                acc = if f == "gcd" { acc.gcd(&b) } else { acc.lcm(&b) };
            }
            Ok(Val::N(Num::Q(BigRational::from_integer(acc))))
        }
        "mod" if v.len() == 2 => match (&v[0], &v[1]) {
            (Val::N(a), Val::N(b)) => {
                if b.is_zero() {
                    return Err("mod by zero".into());
                }
                match (a, b) {
                    (Num::Q(x), Num::Q(y)) => Ok(Val::N(Num::Q(x - y * (x / y).floor()))),
                    _ => Ok(Val::N(Num::R(a.to_f64().rem_euclid(b.to_f64())))),
                }
            }
            _ => Err("mod needs numbers".into()),
        },
        "min" | "max" => {
            let ns = nums(v);
            let mut best = ns.first().cloned().ok_or("needs numbers")?;
            for n in &ns[1..] {
                let d = n.sub(&best)?.sign();
                if (f == "min" && d < 0) || (f == "max" && d > 0) {
                    best = n.clone();
                }
            }
            Ok(Val::N(best))
        }
        "total" | "sum" => {
            let mut s = Num::zero();
            for n in nums(v) {
                s = s.add(&n)?;
            }
            Ok(Val::N(s))
        }
        "mean" => {
            let ns = nums(v);
            let mut s = Num::zero();
            for n in &ns {
                s = s.add(n)?;
            }
            Ok(Val::N(s.div(&Num::int(ns.len() as i64))?))
        }
        "median" => {
            let mut ns = nums(v);
            if ns.is_empty() {
                return Err("median needs numbers".into());
            }
            ns.sort_by(|a, b| a.to_f64().total_cmp(&b.to_f64()));
            let m = ns.len() / 2;
            Ok(Val::N(if ns.len() % 2 == 1 { ns[m].clone() } else { ns[m - 1].add(&ns[m])?.div(&Num::int(2))? }))
        }
        "stdev" | "stdevp" | "var" | "varp" => {
            let ns = nums(v);
            let n = ns.len();
            let sample = f == "stdev" || f == "var";
            if n < if sample { 2 } else { 1 } {
                return Err("needs more numbers".into());
            }
            let mut m = Num::zero();
            for x in &ns {
                m = m.add(x)?;
            }
            m = m.div(&Num::int(n as i64))?;
            let mut ss = Num::zero();
            for x in &ns {
                let d = x.sub(&m)?;
                ss = ss.add(&d.mul(&d)?)?;
            }
            let var = ss.div(&Num::int(if sample { n as i64 - 1 } else { n as i64 }))?;
            Ok(Val::N(if f.starts_with("var") { var } else { var.pow(&Num::ratio(1, 2))? }))
        }
        "sort" => {
            let mut ns = nums(v);
            ns.sort_by(|a, b| a.to_f64().total_cmp(&b.to_f64()));
            Ok(Val::List(ns.into_iter().map(Val::N).collect()))
        }
        "count" => Ok(Val::N(Num::int(nums(v).len() as i64))),
        "corr" if v.len() == 2 => {
            let (xs, ys) = (nums(&v[..1]), nums(&v[1..]));
            if xs.len() != ys.len() || xs.len() < 2 {
                return Err("corr(X, Y): two lists of the same length".into());
            }
            let n = xs.len() as f64;
            let (mx, my) = (xs.iter().map(Num::to_f64).sum::<f64>() / n, ys.iter().map(Num::to_f64).sum::<f64>() / n);
            let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
            for (a, b) in xs.iter().zip(&ys) {
                let (dx, dy) = (a.to_f64() - mx, b.to_f64() - my);
                sxy += dx * dy;
                sxx += dx * dx;
                syy += dy * dy;
            }
            Ok(Val::N(Num::R(sxy / (sxx * syy).sqrt())))
        }
        "normalpdf" => {
            let a: Vec<f64> = nums(v).iter().map(Num::to_f64).collect();
            let (x, mu, sd) = (a.first().copied().ok_or("normalpdf(x, μ, σ)")?, a.get(1).copied().unwrap_or(0.0), a.get(2).copied().unwrap_or(1.0));
            Ok(Val::N(Num::R((-0.5 * ((x - mu) / sd).powi(2)).exp() / (sd * (2.0 * std::f64::consts::PI).sqrt()))))
        }
        "normalcdf" => {
            let a: Vec<f64> = nums(v).iter().map(Num::to_f64).collect();
            if a.len() < 2 {
                return Err("normalcdf(a, b, μ, σ)".into());
            }
            let (lo, hi, mu, sd) = (a[0], a[1], a.get(2).copied().unwrap_or(0.0), a.get(3).copied().unwrap_or(1.0));
            let phi = |z: f64| 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
            Ok(Val::N(Num::R(phi((hi - mu) / sd) - phi((lo - mu) / sd))))
        }
        "binompdf" | "binomcdf" => {
            // binompdf(n, p, k): exact when p is exact
            let a = nums(v);
            if a.len() != 3 {
                return Err(format!("{f}(n, p, k)").into());
            }
            let n = a[0].as_i64().filter(|n| (0..=2000).contains(n)).ok_or("n: a whole number")?;
            let k = a[2].as_i64().ok_or("k: a whole number")?;
            let p = a[1].clone();
            let q = Num::one().sub(&p)?;
            let term = |i: i64| -> Result<Num, String> {
                let mut c = BigInt::from(1);
                for j in 0..i {
                    c = c * (n - j) / (j + 1);
                }
                Num::Q(BigRational::from_integer(c)).mul(&p.pow(&Num::int(i))?)?.mul(&q.pow(&Num::int(n - i))?)
            };
            if f == "binompdf" {
                return Ok(Val::N(if (0..=n).contains(&k) { term(k)? } else { Num::zero() }));
            }
            let mut acc = Num::zero();
            for i in 0..=k.min(n) {
                acc = acc.add(&term(i)?)?;
            }
            Ok(Val::N(acc))
        }
        "poissonpdf" | "poissoncdf" => {
            let a: Vec<f64> = nums(v).iter().map(Num::to_f64).collect();
            if a.len() != 2 {
                return Err(format!("{f}(λ, k)").into());
            }
            let (l, k) = (a[0], a[1].floor() as i64);
            let pdf = |i: i64| (-l + i as f64 * l.ln() - (1..=i).map(|j| (j as f64).ln()).sum::<f64>()).exp();
            Ok(Val::N(Num::R(if f == "poissonpdf" { pdf(k) } else { (0..=k).map(pdf).sum() })))
        }
        "bitand" | "bitor" | "bitxor" | "shl" | "shr" if v.len() == 2 => {
            let (a, b) = (int_arg(&v[0], f)?, int_arg(&v[1], f)?);
            let r = match f {
                "bitand" => a & b,
                "bitor" => a | b,
                "bitxor" => a ^ b,
                "shl" => a << b.to_usize().filter(|s| *s < 4096).ok_or("shift too large")?,
                _ => a >> b.to_usize().filter(|s| *s < 4096).ok_or("shift too large")?,
            };
            Ok(Val::N(Num::Q(BigRational::from_integer(r))))
        }
        "length" => match one(f)? {
            Val::List(l) => Ok(Val::N(Num::int(l.len() as i64))),
            _ => Err("length needs a list".into()),
        },
        "det" => Ok(Val::N(mat("det")?.det()?)),
        "inv" => Ok(Val::Mat(mat("inv")?.inv()?)),
        "transpose" => Ok(Val::Mat(mat("transpose")?.transpose())),
        "rref" => Ok(Val::Mat(mat("rref")?.rref()?.0)),
        "rank" => Ok(Val::N(Num::int(mat("rank")?.rref()?.1 as i64))),
        "tr" => {
            let m = mat("tr")?;
            if m.r != m.c {
                return Err("trace needs a square matrix".into());
            }
            let mut s = Num::zero();
            for i in 0..m.r {
                s = s.add(m.at(i, i))?;
            }
            Ok(Val::N(s))
        }
        "eye" => {
            let n = int_arg(one(f)?, f)?.to_usize().filter(|n| (1..=50).contains(n)).ok_or("eye(n) needs 1 ≤ n ≤ 50")?;
            Ok(Val::Mat(Mat::eye(n)))
        }
        "deg" => real(&f64::to_degrees),
        "rad" => real(&f64::to_radians),
        _ => Err(format!("{f}: unknown function or wrong arguments").into()),
    }
}

fn erf(x: f64) -> f64 {
    // Abramowitz–Stegun 7.1.26 refined with a few Newton steps on the series is overkill here; use a high-precision
    // continued expansion (|error| < 1.2e-7) — enough for probabilities shown to 7 digits.
    let t = 1.0 / (1.0 + 0.5 * x.abs());
    let y = 1.0 - t * (-x * x - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806
        + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))))).exp();
    if x >= 0.0 { y } else { -y }
}

/// Plain text of a value.
pub fn plain(v: &Val, digits: usize) -> String {
    match v {
        Val::N(n) => n.plain(digits),
        Val::Bool(b) => if *b { "true" } else { "false" }.into(),
        Val::List(l) => format!("[{}]", l.iter().map(|x| plain(x, digits)).collect::<Vec<_>>().join(", ")),
        Val::Tuple(l) => format!("({})", l.iter().map(|x| plain(x, digits)).collect::<Vec<_>>().join(", ")),
        Val::Mat(m) => {
            let rows: Vec<String> = (0..m.r)
                .map(|i| (0..m.c).map(|j| m.at(i, j).plain(digits)).collect::<Vec<_>>().join(", "))
                .collect();
            format!("[{}]", rows.join("; "))
        }
    }
}

/// Decimal approximation when it adds something to the exact form.
pub fn approx(v: &Val, digits: usize) -> Option<String> {
    match v {
        Val::N(n) => n.approx(digits),
        Val::List(l) | Val::Tuple(l) if l.iter().any(|x| approx(x, digits).is_some()) => {
            let inner: Vec<String> = l
                .iter()
                .map(|x| match x {
                    Val::N(n) => fmt_float(n.to_f64(), digits),
                    o => plain(o, digits),
                })
                .collect();
            Some(if matches!(v, Val::List(_)) { format!("[{}]", inner.join(", ")) } else { format!("({})", inner.join(", ")) })
        }
        _ => None,
    }
}
