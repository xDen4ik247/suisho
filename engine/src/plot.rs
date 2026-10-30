//! The grapher's engine side: every graph row is compiled to a fast f64 function, then sampled for the
//! window's current view (plus a margin, so panning has geometry ready) into SVG paths in that view's pixel space:
//!   y = f(x) / x = g(y)   adaptive sampling (finer where the curve bends), breaks at jumps and asymptotes
//!   r = f(θ), (f(t), g(t)) adaptive in the parameter
//!   F(x, y) = G(x, y)      marching squares on a pixel grid, sign changes through poles rejected
//!   inequalities           filled regions (explicit: down/up to the edge; implicit: cell runs) + dashed edge for < >
//!   points, lists of points
//! Points of interest (roots, extrema, y-intercepts, intersections of curves) are refined to full precision.

use crate::eval::{Env, Val};
use crate::parse::{Ast, Rel};
use serde_json::{json, Value};
use std::collections::HashMap;

// ------------------------------------------------------------------ compiled functions

#[derive(Clone, Debug)]
pub enum F {
    C(f64),
    X,
    Y,
    T, // t, θ (one parameter per row)
    Add(Box<F>, Box<F>),
    Sub(Box<F>, Box<F>),
    Mul(Box<F>, Box<F>),
    Div(Box<F>, Box<F>),
    Pow(Box<F>, Box<F>),
    Neg(Box<F>),
    Fn(fn(f64) -> f64, Box<F>),
    Fn2(fn(f64, f64) -> f64, Box<F>, Box<F>),
    Cmp(Rel, Box<F>, Box<F>),           // 1 or NaN
    Pw(Vec<(F, F)>, Option<Box<F>>),     // piecewise: (condition, value)
    Restrict(Box<F>, Vec<F>),
    D(Box<F>), // d/dx (central difference)
}

impl F {
    #[inline]
    pub fn ev(&self, x: f64, y: f64, t: f64) -> f64 {
        match self {
            F::C(c) => *c,
            F::X => x,
            F::Y => y,
            F::T => t,
            F::Add(a, b) => a.ev(x, y, t) + b.ev(x, y, t),
            F::Sub(a, b) => a.ev(x, y, t) - b.ev(x, y, t),
            F::Mul(a, b) => a.ev(x, y, t) * b.ev(x, y, t),
            F::Div(a, b) => {
                let d = b.ev(x, y, t);
                if d == 0.0 {
                    f64::NAN
                } else {
                    a.ev(x, y, t) / d
                }
            }
            F::Pow(a, b) => pw(a.ev(x, y, t), b, x, y, t),
            F::Neg(a) => -a.ev(x, y, t),
            F::Fn(f, a) => f(a.ev(x, y, t)),
            F::Fn2(f, a, b) => f(a.ev(x, y, t), b.ev(x, y, t)),
            F::Cmp(r, a, b) => {
                let (u, v) = (a.ev(x, y, t), b.ev(x, y, t));
                let ok = match r {
                    Rel::Lt => u < v,
                    Rel::Le => u <= v,
                    Rel::Gt => u > v,
                    Rel::Ge => u >= v,
                    Rel::Eq => (u - v).abs() <= 1e-9 * (1.0 + u.abs()),
                    Rel::Ne => u != v,
                };
                if ok {
                    1.0
                } else {
                    f64::NAN
                }
            }
            F::Pw(cases, other) => {
                for (c, v) in cases {
                    if !c.ev(x, y, t).is_nan() {
                        return v.ev(x, y, t);
                    }
                }
                other.as_ref().map(|o| o.ev(x, y, t)).unwrap_or(f64::NAN)
            }
            F::Restrict(e, conds) => {
                if conds.iter().all(|c| !c.ev(x, y, t).is_nan()) {
                    e.ev(x, y, t)
                } else {
                    f64::NAN
                }
            }
            F::D(e) => {
                let h = 1e-5 * (1.0 + x.abs());
                (e.ev(x + h, y, t) - e.ev(x - h, y, t)) / (2.0 * h)
            }
        }
    }
    fn uses(&self, v: char) -> bool {
        match self {
            F::X => v == 'x',
            F::Y => v == 'y',
            F::T => v == 't',
            F::C(_) => false,
            F::Add(a, b) | F::Sub(a, b) | F::Mul(a, b) | F::Div(a, b) | F::Pow(a, b) | F::Fn2(_, a, b) | F::Cmp(_, a, b) => a.uses(v) || b.uses(v),
            F::Neg(a) | F::Fn(_, a) | F::D(a) => a.uses(v),
            F::Pw(c, o) => c.iter().any(|(a, b)| a.uses(v) || b.uses(v)) || o.as_ref().is_some_and(|o| o.uses(v)),
            F::Restrict(e, c) => e.uses(v) || c.iter().any(|x| x.uses(v)),
        }
    }
}

#[inline]
fn pw(b: f64, e: &F, x: f64, y: f64, t: f64) -> f64 {
    // odd roots of negatives stay real: x^(1/3)
    let ev = e.ev(x, y, t);
    if b < 0.0 && ev.fract() != 0.0 {
        if let F::Div(n, d) = e {
            if let (F::C(n), F::C(d)) = (&**n, &**d) {
                if d.fract() == 0.0 && (*d as i64) % 2 != 0 {
                    let r = (-b).powf(ev);
                    return if (*n as i64) % 2 != 0 { -r } else { r };
                }
            }
        }
        return f64::NAN;
    }
    b.powf(ev)
}

pub struct Compiler<'a> {
    pub env: &'a mut Env,
    pub degrees: bool,
    /// which letter is the parameter (t or theta)
    pub param: &'static str,
}

fn trig_in(deg: bool) -> f64 {
    if deg {
        std::f64::consts::PI / 180.0
    } else {
        1.0
    }
}

impl Compiler<'_> {
    pub fn compile(&mut self, a: &Ast, loc: &HashMap<String, F>) -> Result<F, String> {
        let b = |f: F| Box::new(f);
        Ok(match a {
            Ast::Num(s) => F::C(s.parse::<f64>().map_err(|_| format!("bad number {s}"))?),
            Ast::Var(n) => {
                if let Some(f) = loc.get(n) {
                    return Ok(f.clone());
                }
                match n.as_str() {
                    "x" => F::X,
                    "y" => F::Y,
                    "t" if self.param == "t" => F::T,
                    "theta" if self.param == "theta" => F::T,
                    "pi" => F::C(std::f64::consts::PI),
                    "e" if !self.env.vars.contains_key("e") => F::C(std::f64::consts::E),
                    "tau" => F::C(std::f64::consts::TAU),
                    _ => match self.env.vars.get(n) {
                        Some(Val::N(v)) => F::C(v.to_f64()),
                        Some(_) => return Err(format!("{n} is not a number")),
                        None => return Err(format!("{n} is not defined")),
                    },
                }
            }
            Ast::Neg(x) => F::Neg(b(self.compile(x, loc)?)),
            Ast::Add(x, y) => F::Add(b(self.compile(x, loc)?), b(self.compile(y, loc)?)),
            Ast::Sub(x, y) => F::Sub(b(self.compile(x, loc)?), b(self.compile(y, loc)?)),
            Ast::Mul(x, y, _) => F::Mul(b(self.compile(x, loc)?), b(self.compile(y, loc)?)),
            Ast::Div(x, y) => F::Div(b(self.compile(x, loc)?), b(self.compile(y, loc)?)),
            Ast::Pow(x, y) => {
                let e = self.compile(y, loc)?;
                // keep 1/3 visible as a fraction so odd roots of negatives work
                let e = match (&**y, &e) {
                    (Ast::Div(n, d), _) => match (&**n, &**d) {
                        (Ast::Num(n), Ast::Num(d)) => F::Div(b(F::C(n.parse().unwrap_or(1.0))), b(F::C(d.parse().unwrap_or(1.0)))),
                        _ => e,
                    },
                    _ => e,
                };
                F::Pow(b(self.compile(x, loc)?), b(e))
            }
            Ast::Abs(x) => F::Fn(f64::abs, b(self.compile(x, loc)?)),
            Ast::Fact(x) => F::Fn(gamma1, b(self.compile(x, loc)?)),
            Ast::Deg(x) => {
                let v = self.compile(x, loc)?;
                if self.degrees {
                    v
                } else {
                    F::Mul(b(v), b(F::C(std::f64::consts::PI / 180.0)))
                }
            }
            Ast::Piecewise(cases, other) => {
                let mut cs = Vec::new();
                for (c, v) in cases {
                    cs.push((self.cond(c, loc)?, self.compile(v, loc)?));
                }
                let o = match other {
                    Some(o) => Some(b(self.compile(o, loc)?)),
                    None => None,
                };
                F::Pw(cs, o)
            }
            Ast::Restrict(e, conds) => {
                let e = self.compile(e, loc)?;
                let cs = conds.iter().map(|c| self.cond(c, loc)).collect::<Result<_, _>>()?;
                F::Restrict(b(e), cs)
            }
            Ast::Call(f, args) => {
                if let Some((params, body)) = self.env.funcs.get(f).cloned() {
                    if params.len() != args.len() {
                        return Err(format!("{f} takes {} argument(s)", params.len()));
                    }
                    let mut l2 = loc.clone();
                    for (p, x) in params.iter().zip(args) {
                        let v = self.compile(x, loc)?;
                        l2.insert(p.clone(), v);
                    }
                    return self.compile(&body, &l2);
                }
                let k = trig_in(self.degrees);
                let one = |me: &mut Self| -> Result<F, String> {
                    if args.len() != 1 {
                        return Err(format!("{f} takes one argument"));
                    }
                    me.compile(&args[0], loc)
                };
                let ang = |x: F| if k == 1.0 { x } else { F::Mul(Box::new(x), Box::new(F::C(k))) };
                let fns: Option<fn(f64) -> f64> = match f.as_str() {
                    "sinh" => Some(f64::sinh),
                    "cosh" => Some(f64::cosh),
                    "tanh" => Some(f64::tanh),
                    "arcsinh" => Some(f64::asinh),
                    "arccosh" => Some(f64::acosh),
                    "arctanh" => Some(f64::atanh),
                    "exp" => Some(f64::exp),
                    "ln" => Some(|x| if x > 0.0 { x.ln() } else { f64::NAN }),
                    "lg" => Some(|x| if x > 0.0 { x.log10() } else { f64::NAN }),
                    "sqrt" => Some(|x| if x >= 0.0 { x.sqrt() } else { f64::NAN }),
                    "cbrt" => Some(f64::cbrt),
                    "abs" => Some(f64::abs),
                    "floor" => Some(f64::floor),
                    "ceil" => Some(f64::ceil),
                    "round" => Some(f64::round),
                    "sign" => Some(f64::signum),
                    "frac" => Some(|x| x - x.floor()),
                    "fact" => Some(gamma1),
                    _ => None,
                };
                if let Some(g) = fns {
                    return Ok(F::Fn(g, b(one(self)?)));
                }
                let trig: Option<fn(f64) -> f64> = match f.as_str() {
                    "sin" => Some(f64::sin),
                    "cos" => Some(f64::cos),
                    "tan" => Some(f64::tan),
                    "cot" => Some(|x| 1.0 / x.tan()),
                    "sec" => Some(|x| 1.0 / x.cos()),
                    "csc" => Some(|x| 1.0 / x.sin()),
                    _ => None,
                };
                if let Some(g) = trig {
                    return Ok(F::Fn(g, b(ang(one(self)?))));
                }
                let inv: Option<fn(f64) -> f64> = match f.as_str() {
                    "arcsin" => Some(f64::asin),
                    "arccos" => Some(f64::acos),
                    "arctan" => Some(f64::atan),
                    "arccot" => Some(|x| std::f64::consts::FRAC_PI_2 - x.atan()),
                    _ => None,
                };
                if let Some(g) = inv {
                    let v = F::Fn(g, b(one(self)?));
                    return Ok(if k == 1.0 { v } else { F::Mul(b(v), b(F::C(1.0 / k))) });
                }
                match (f.as_str(), args.len()) {
                    ("log", 1) => F::Fn(|x| if x > 0.0 { x.log10() } else { f64::NAN }, b(self.compile(&args[0], loc)?)),
                    ("log", 2) => F::Fn2(|x, base| if x > 0.0 && base > 0.0 && base != 1.0 { x.ln() / base.ln() } else { f64::NAN },
                                         b(self.compile(&args[0], loc)?), b(self.compile(&args[1], loc)?)),
                    ("root", 2) => F::Pow(b(self.compile(&args[0], loc)?), b(F::Div(b(F::C(1.0)), b(self.compile(&args[1], loc)?)))),
                    ("mod", 2) => F::Fn2(|a, m| a.rem_euclid(m), b(self.compile(&args[0], loc)?), b(self.compile(&args[1], loc)?)),
                    ("min", 2) => F::Fn2(f64::min, b(self.compile(&args[0], loc)?), b(self.compile(&args[1], loc)?)),
                    ("max", 2) => F::Fn2(f64::max, b(self.compile(&args[0], loc)?), b(self.compile(&args[1], loc)?)),
                    _ => return Err(format!("{f} can't be graphed yet")),
                }
            }
            Ast::Rel(..) => return Err("a relation inside an expression".into()),
            Ast::List(_) | Ast::Matrix(_) | Ast::Tuple(_) => return Err("lists, points and matrices are not functions".into()),
        })
    }

    fn cond(&mut self, c: &Ast, loc: &HashMap<String, F>) -> Result<F, String> {
        match c {
            Ast::Rel(first, chain) => {
                let mut prev = self.compile(first, loc)?;
                let mut acc: Option<F> = None;
                for (r, e) in chain {
                    let cur = self.compile(e, loc)?;
                    let cmp = F::Cmp(*r, Box::new(prev), Box::new(cur.clone()));
                    acc = Some(match acc {
                        None => cmp,
                        Some(a) => F::Mul(Box::new(a), Box::new(cmp)),
                    });
                    prev = cur;
                }
                acc.ok_or_else(|| "empty condition".into())
            }
            _ => Err("a condition needs <, >, ≤, ≥ or =".into()),
        }
    }
}

fn gamma1(x: f64) -> f64 {
    // x! = Γ(x+1) (Lanczos)
    let z = x + 1.0;
    if z < 0.5 {
        // reflection: Γ(z) = π / (sin(πz)·Γ(1 − z)), and Γ(1 − z) = gamma1(−z)
        return std::f64::consts::PI / ((std::f64::consts::PI * z).sin() * gamma1(-z));
    }
    const G: [f64; 9] = [0.99999999999980993, 676.5203681218851, -1259.1392167224028, 771.32342877765313, -176.61502916214059,
                         12.507343278686905, -0.13857109526572012, 9.9843695780195716e-6, 1.5056327351493116e-7];
    let z = z - 1.0;
    let mut a = G[0];
    let tt = z + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (z + i as f64);
    }
    (2.0 * std::f64::consts::PI).sqrt() * tt.powf(z + 0.5) * (-tt).exp() * a
}

// ------------------------------------------------------------------ graph kinds

enum Kind {
    Explicit(F, Rel),  // y (rel) f(x)
    ExplicitY(F, Rel), // x (rel) g(y)
    Polar(F),          // r = f(θ)
    Param(F, F, f64, f64),
    Implicit(F, Rel), // G(x,y) (rel) 0
    Points(Vec<(f64, f64)>),
    Area(F, f64, f64), // ∫ₐᵇ f: the curve on [a, b] and the region down to the x-axis
    Poly(Vec<(f64, f64)>, bool), // a polyline through points (closed: a polygon)
}

fn kind_of(a: &Ast, env: &mut Env, degrees: bool) -> Result<Option<Kind>, String> {
    let no = HashMap::new();
    let (core, conds): (&Ast, Vec<&Ast>) = match a {
        Ast::Restrict(e, c) => (e, c.iter().collect()),
        o => (o, vec![]),
    };
    let wrap = |f: F, cm: &mut Compiler| -> Result<F, String> {
        if conds.is_empty() {
            return Ok(f);
        }
        let cs = conds.iter().map(|c| cm.cond(c, &no)).collect::<Result<Vec<_>, _>>()?;
        Ok(F::Restrict(Box::new(f), cs))
    };
    let has = |x: &Ast, v: &str| ast_uses(x, v);
    match core {
        Ast::Rel(l, chain) if chain.len() == 1 => {
            let (r, rhs) = (chain[0].0, &chain[0].1);
            let mut cm = Compiler { env, degrees, param: "theta" };
            if matches!(&**l, Ast::Var(v) if v == "y") && !has(rhs, "y") {
                let f = cm.compile(rhs, &no)?;
                return Ok(Some(Kind::Explicit(wrap(f, &mut cm)?, r)));
            }
            if matches!(rhs, Ast::Var(v) if v == "y") && !has(l, "y") {
                let f = cm.compile(l, &no)?;
                return Ok(Some(Kind::Explicit(wrap(f, &mut cm)?, flip(r))));
            }
            if matches!(&**l, Ast::Var(v) if v == "x") && !has(rhs, "x") && has(rhs, "y") {
                let f = cm.compile(rhs, &no)?;
                return Ok(Some(Kind::ExplicitY(wrap(f, &mut cm)?, r)));
            }
            if matches!(&**l, Ast::Var(v) if v == "x") && !has(rhs, "x") && !has(rhs, "y") {
                let f = cm.compile(rhs, &no)?;
                return Ok(Some(Kind::ExplicitY(wrap(f, &mut cm)?, r)));
            }
            if matches!(&**l, Ast::Var(v) if v == "r") && r == Rel::Eq && !has(rhs, "x") && !has(rhs, "y") {
                let f = cm.compile(rhs, &no)?;
                return Ok(Some(Kind::Polar(wrap(f, &mut cm)?)));
            }
            let g = F::Sub(Box::new(cm.compile(l, &no)?), Box::new(cm.compile(rhs, &no)?));
            if !g.uses('x') && !g.uses('y') {
                return Ok(None);
            }
            Ok(Some(Kind::Implicit(wrap(g, &mut cm)?, r)))
        }
        Ast::Tuple(items) if items.len() == 2 => {
            let mut cm = Compiler { env, degrees, param: "t" };
            let fx = cm.compile(&items[0], &no)?;
            let fy = cm.compile(&items[1], &no)?;
            if fx.uses('t') || fy.uses('t') {
                // a range from the restriction {a < t < b}, else 0 … 2π
                let (mut t0, mut t1) = (0.0, std::f64::consts::TAU);
                for c in &conds {
                    if let Ast::Rel(first, ch) = c {
                        if ch.len() == 2 && matches!(&ch[0].1, Ast::Var(v) if v == "t") {
                            let lo = cm.compile(first, &no)?.ev(0.0, 0.0, 0.0);
                            let hi = cm.compile(&ch[1].1, &no)?.ev(0.0, 0.0, 0.0);
                            if lo.is_finite() && hi.is_finite() && hi > lo {
                                t0 = lo;
                                t1 = hi;
                            }
                        }
                    }
                }
                return Ok(Some(Kind::Param(fx, fy, t0, t1)));
            }
            if fx.uses('x') || fx.uses('y') || fy.uses('x') || fy.uses('y') {
                return Err("a point can't depend on x or y".into());
            }
            Ok(Some(Kind::Points(vec![(fx.ev(0.0, 0.0, 0.0), fy.ev(0.0, 0.0, 0.0))])))
        }
        Ast::List(items) if items.iter().all(|i| matches!(i, Ast::Tuple(p) if p.len() == 2)) && !items.is_empty() => {
            let mut pts = Vec::new();
            for i in items {
                if let Val::Tuple(p) = env.eval(i, &HashMap::new()).map_err(|_| "a point can't be evaluated".to_string())? {
                    if let (Val::N(x), Val::N(y)) = (&p[0], &p[1]) {
                        pts.push((x.to_f64(), y.to_f64()));
                    }
                }
            }
            Ok(Some(Kind::Points(pts)))
        }
        Ast::Call(f, args) if matches!(f.as_str(), "integrate" | "int") && args.len() == 3 => {
            let mut cm = Compiler { env, degrees, param: "theta" };
            let g = cm.compile(&args[0], &no)?;
            let a = cm.compile(&args[1], &no)?.ev(0.0, 0.0, 0.0);
            let b = cm.compile(&args[2], &no)?.ev(0.0, 0.0, 0.0);
            if !(a.is_finite() && b.is_finite()) {
                return Err("the bounds must be numbers".into());
            }
            Ok(Some(Kind::Area(g, a.min(b), a.max(b))))
        }
        Ast::Call(f, args) if matches!(f.as_str(), "diff" | "derivative") && !args.is_empty() => {
            let mut cm = Compiler { env, degrees, param: "theta" };
            Ok(Some(Kind::Explicit(F::D(Box::new(cm.compile(&args[0], &no)?)), Rel::Eq)))
        }
        Ast::Call(f, args) if matches!(f.as_str(), "line" | "triangle" | "midpoint") && args.len() >= 2 => {
            let mut pts = Vec::new();
            for a in args {
                match env.eval(a, &HashMap::new()) {
                    Ok(Val::Tuple(p)) if p.len() == 2 => {
                        if let (Val::N(x), Val::N(y)) = (&p[0], &p[1]) {
                            pts.push((x.to_f64(), y.to_f64()));
                        }
                    }
                    _ => return Ok(None),
                }
            }
            if f == "line" && pts.len() == 2 {
                // extend the segment across the view
                let ((x1, y1), (x2, y2)) = (pts[0], pts[1]);
                let (dx, dy) = (x2 - x1, y2 - y1);
                let n = (dx * dx + dy * dy).sqrt().max(1e-300);
                let big = 1e3 * (1.0 + x1.abs() + y1.abs());
                return Ok(Some(Kind::Poly(vec![(x1 - big * dx / n, y1 - big * dy / n), (x1 + big * dx / n, y1 + big * dy / n)], false)));
            }
            if f == "midpoint" {
                let m = ((pts[0].0 + pts[1].0) / 2.0, (pts[0].1 + pts[1].1) / 2.0);
                return Ok(Some(Kind::Points(vec![m])));
            }
            Ok(Some(Kind::Poly(pts, true)))
        }
        Ast::Call(f, args) if matches!(f.as_str(), "analyze" | "investigate") && args.len() == 1 => {
            let mut cm = Compiler { env, degrees, param: "theta" };
            Ok(Some(Kind::Explicit(cm.compile(&args[0], &no)?, Rel::Eq)))
        }
        Ast::Call(f, args) if f == "tangent" && args.len() == 2 => {
            let mut cm = Compiler { env, degrees, param: "theta" };
            let g = cm.compile(&args[0], &no)?;
            let a = cm.compile(&args[1], &no)?.ev(0.0, 0.0, 0.0);
            let (fa, k) = (g.ev(a, 0.0, 0.0), F::D(Box::new(g)).ev(a, 0.0, 0.0));
            if !(fa.is_finite() && k.is_finite()) {
                return Err("no tangent there (the function or its derivative is undefined)".into());
            }
            // y = f(a) + f′(a)(x − a)
            Ok(Some(Kind::Explicit(F::Add(Box::new(F::C(fa)), Box::new(F::Mul(Box::new(F::C(k)), Box::new(F::Sub(Box::new(F::X), Box::new(F::C(a))))))), Rel::Eq)))
        }
        Ast::Rel(..) => Ok(None),
        e => {
            if has(e, "y") || !has(e, "x") {
                return Ok(None);
            }
            let mut cm = Compiler { env, degrees, param: "theta" };
            let f = cm.compile(e, &no)?;
            Ok(Some(Kind::Explicit(wrap(f, &mut cm)?, Rel::Eq)))
        }
    }
}

fn flip(r: Rel) -> Rel {
    match r {
        Rel::Lt => Rel::Gt,
        Rel::Gt => Rel::Lt,
        Rel::Le => Rel::Ge,
        Rel::Ge => Rel::Le,
        o => o,
    }
}

pub fn ast_uses(a: &Ast, v: &str) -> bool {
    match a {
        Ast::Var(n) => n == v,
        Ast::Num(_) => false,
        Ast::Neg(x) | Ast::Fact(x) | Ast::Deg(x) | Ast::Abs(x) => ast_uses(x, v),
        Ast::Add(x, y) | Ast::Sub(x, y) | Ast::Mul(x, y, _) | Ast::Div(x, y) | Ast::Pow(x, y) => ast_uses(x, v) || ast_uses(y, v),
        Ast::Call(_, a) | Ast::List(a) | Ast::Tuple(a) => a.iter().any(|x| ast_uses(x, v)),
        Ast::Matrix(r) => r.iter().flatten().any(|x| ast_uses(x, v)),
        Ast::Rel(f, ch) => ast_uses(f, v) || ch.iter().any(|(_, e)| ast_uses(e, v)),
        Ast::Piecewise(c, o) => c.iter().any(|(a, b)| ast_uses(a, v) || ast_uses(b, v)) || o.as_ref().is_some_and(|o| ast_uses(o, v)),
        Ast::Restrict(e, c) => ast_uses(e, v) || c.iter().any(|x| ast_uses(x, v)),
    }
}

// ------------------------------------------------------------------ view and sampling

#[derive(Clone, Copy)]
pub struct View {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    pub w: f64,
    pub h: f64,
}
impl View {
    fn px(&self, x: f64) -> f64 {
        (x - self.x0) / (self.x1 - self.x0) * self.w
    }
    fn py(&self, y: f64) -> f64 {
        (self.y1 - y) / (self.y1 - self.y0) * self.h
    }
    fn ux(&self) -> f64 {
        (self.x1 - self.x0) / self.w
    }
    fn uy(&self) -> f64 {
        (self.y1 - self.y0) / self.h
    }
}

struct PathBuf {
    s: String,
    pen: bool,
}
impl PathBuf {
    fn new() -> Self {
        PathBuf { s: String::new(), pen: false }
    }
    fn pt(&mut self, x: f64, y: f64) {
        use std::fmt::Write;
        let _ = write!(self.s, "{}{:.1} {:.1} ", if self.pen { "L" } else { "M" }, x, y);
        self.pen = true;
    }
    fn lift(&mut self) {
        self.pen = false;
    }
}

/// y = f(x) sampled in pixel space over [xa, xb]; returns paths and the math-space samples (for POIs).
fn sample_fx(f: &dyn Fn(f64) -> f64, v: &View, xa: f64, xb: f64, swap: bool) -> (Vec<Vec<(f64, f64)>>, Vec<(f64, f64)>) {
    // in "swap" mode the roles of x and y are exchanged (x = g(y))
    let (ppx, ppy): (Box<dyn Fn(f64) -> f64>, Box<dyn Fn(f64) -> f64>) = if swap {
        (Box::new(|u| v.py(u)), Box::new(|w| v.px(w)))
    } else {
        (Box::new(|u| v.px(u)), Box::new(|w| v.py(w)))
    };
    let span = if swap { v.h } else { v.w };
    let lim = 8.0 * span.max(v.h).max(v.w);
    let n0 = ((xb - xa).abs() / (if swap { v.uy() } else { v.ux() }) / 4.0).clamp(64.0, 4000.0) as usize;
    let mut raw: Vec<(f64, f64)> = Vec::with_capacity(n0 * 2);
    let mut lines: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let mut prev = (xa, f(xa));
    raw.push(prev);
    let tol = 0.35;
    fn refine(f: &dyn Fn(f64) -> f64, a: (f64, f64), b: (f64, f64), depth: u32, ppx: &dyn Fn(f64) -> f64, ppy: &dyn Fn(f64) -> f64,
              tol: f64, out: &mut Vec<(f64, f64, bool)>) {
        let m = (a.0 + b.0) / 2.0;
        let fm = f(m);
        let (pa, pb, pm) = (ppy(a.1), ppy(b.1), ppy(fm));
        let finite = a.1.is_finite() && b.1.is_finite() && fm.is_finite();
        let bent = finite && ((pm - (pa + pb) / 2.0).abs() > tol || (pa - pb).abs() > 24.0);
        let edge = a.1.is_finite() != b.1.is_finite() || (fm.is_finite() != a.1.is_finite());
        let small = (ppx(b.0) - ppx(a.0)).abs() <= 0.05;
        if depth < 12 && (bent || edge) && !small {
            refine(f, a, (m, fm), depth + 1, ppx, ppy, tol, out);
            refine(f, (m, fm), b, depth + 1, ppx, ppy, tol, out);
            return;
        }
        // a jump that never settles: a discontinuity (asymptote / step) — break the line here
        let jump = finite && (pa - pb).abs() > 30.0 && (depth >= 12 || small);
        out.push((b.0, b.1, jump));
    }
    let mut seg: Vec<(f64, f64, bool)> = Vec::new();
    for i in 1..=n0 {
        let x = xa + (xb - xa) * i as f64 / n0 as f64;
        let p = (x, f(x));
        seg.clear();
        refine(f, prev, p, 0, &*ppx, &*ppy, tol, &mut seg);
        for &(sx, sy, jump) in &seg {
            raw.push((sx, sy));
            if !sy.is_finite() || jump {
                if cur.len() > 1 {
                    lines.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
                if jump && sy.is_finite() {
                    cur.push((ppx(sx), ppy(sy).clamp(-lim, lim)));
                }
                continue;
            }
            cur.push((ppx(sx), ppy(sy).clamp(-lim, lim)));
        }
        prev = p;
    }
    if cur.len() > 1 {
        lines.push(cur);
    }
    if swap {
        for l in &mut lines {
            for p in l.iter_mut() {
                *p = (p.1, p.0);
            }
        }
    }
    (lines, raw)
}

fn lines_to_path(lines: &[Vec<(f64, f64)>]) -> String {
    lines_to_path_band(lines, f64::NEG_INFINITY, f64::INFINITY)
}

/// Lines to an SVG path, dropping the stretches that run far above/below the band [lo, hi] (only the step out
/// of the band and back in is kept, so the curve still leaves the screen in the right direction).
fn lines_to_path_band(lines: &[Vec<(f64, f64)>], lo: f64, hi: f64) -> String {
    let mut pb = PathBuf::new();
    for l in lines {
        pb.lift();
        let out = |y: f64| y < lo || y > hi;
        // drop points closer than 0.3 px to the previous one
        let mut last = (f64::NAN, f64::NAN);
        for (i, &(x, y)) in l.iter().enumerate() {
            // outside the band, and so are both neighbours: not drawn
            let prev_out = i == 0 || out(l[i - 1].1);
            let next_out = i + 1 >= l.len() || out(l[i + 1].1);
            if out(y) && prev_out && next_out {
                pb.lift();
                last = (f64::NAN, f64::NAN);
                continue;
            }
            if i + 1 < l.len() && (x - last.0).abs() < 0.3 && (y - last.1).abs() < 0.3 {
                continue;
            }
            pb.pt(x, y);
            last = (x, y);
        }
    }
    pb.s
}

/// A parametric curve (x(t), y(t)) sampled adaptively in t.
fn sample_param(fx: &dyn Fn(f64) -> (f64, f64), v: &View, t0: f64, t1: f64) -> Vec<Vec<(f64, f64)>> {
    let n0 = 400;
    let mut lines = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let to_px = |p: (f64, f64)| (v.px(p.0), v.py(p.1));
    let lim = 8.0 * v.w.max(v.h);
    fn rec(fx: &dyn Fn(f64) -> (f64, f64), ta: f64, tb: f64, pa: (f64, f64), pb: (f64, f64), d: u32, to_px: &dyn Fn((f64, f64)) -> (f64, f64), out: &mut Vec<(f64, f64)>) {
        let tm = (ta + tb) / 2.0;
        let pm = fx(tm);
        let (a, b, m) = (to_px(pa), to_px(pb), to_px(pm));
        let ok = pa.0.is_finite() && pa.1.is_finite() && pb.0.is_finite() && pb.1.is_finite() && pm.0.is_finite() && pm.1.is_finite();
        let dev = ((m.0 - (a.0 + b.0) / 2.0).powi(2) + (m.1 - (a.1 + b.1) / 2.0).powi(2)).sqrt();
        let len = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        if d < 12 && (!ok || dev > 0.3 || len > 12.0) {
            rec(fx, ta, tm, pa, pm, d + 1, to_px, out);
            rec(fx, tm, tb, pm, pb, d + 1, to_px, out);
        } else {
            out.push(pb);
        }
    }
    let mut prev_t = t0;
    let mut prev = fx(t0);
    let mut buf = Vec::new();
    if prev.0.is_finite() && prev.1.is_finite() {
        cur.push(to_px(prev));
    }
    for i in 1..=n0 {
        let t = t0 + (t1 - t0) * i as f64 / n0 as f64;
        let p = fx(t);
        buf.clear();
        rec(fx, prev_t, t, prev, p, 0, &to_px, &mut buf);
        for &q in &buf {
            if q.0.is_finite() && q.1.is_finite() {
                let (x, y) = to_px(q);
                cur.push((x.clamp(-lim, lim), y.clamp(-lim, lim)));
            } else if cur.len() > 1 {
                lines.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
        prev_t = t;
        prev = p;
    }
    if cur.len() > 1 {
        lines.push(cur);
    }
    lines
}

/// Marching squares for G(x,y) = 0 on a `cell`-pixel grid; with `fill`, the cells where G (rel) 0 holds.
fn implicit(g: &F, v: &View, rel: Rel, cell: f64, ox: f64, oy: f64) -> (String, String) {
    let nx = (v.w / cell).ceil() as usize + 1;
    let ny = (v.h / cell).ceil() as usize + 1;
    let mut val = vec![0.0f64; nx * ny];
    for j in 0..ny {
        let y = v.y1 - j as f64 * cell * v.uy();
        for i in 0..nx {
            let x = v.x0 + i as f64 * cell * v.ux();
            val[j * nx + i] = g.ev(x, y, 0.0);
        }
    }
    let at = |i: usize, j: usize| val[j * nx + i];
    let mut pb = PathBuf::new();
    let draw_curve = matches!(rel, Rel::Eq | Rel::Le | Rel::Ge | Rel::Lt | Rel::Gt);
    if draw_curve {
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let c = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                if c.iter().any(|z| !z.is_finite()) {
                    continue;
                }
                let idx = (c[0] > 0.0) as u8 | ((c[1] > 0.0) as u8) << 1 | ((c[2] > 0.0) as u8) << 2 | ((c[3] > 0.0) as u8) << 3;
                if idx == 0 || idx == 15 {
                    continue;
                }
                // a sign change through a pole (1/x = 0) has a huge value in the middle: skip it
                let xm = v.x0 + (i as f64 + 0.5) * cell * v.ux();
                let ym = v.y1 - (j as f64 + 0.5) * cell * v.uy();
                let gm = g.ev(xm, ym, 0.0);
                let maxc = c.iter().fold(0.0f64, |m, z| m.max(z.abs()));
                if !gm.is_finite() || gm.abs() > 4.0 * maxc + 1e-9 {
                    continue;
                }
                let (x0, y0) = (i as f64 * cell + ox, j as f64 * cell + oy);
                let lerp = |a: f64, b: f64| if (b - a).abs() < 1e-300 { 0.5 } else { (a / (a - b)).clamp(0.0, 1.0) };
                let e = [
                    (x0 + lerp(c[0], c[1]) * cell, y0),
                    (x0 + cell, y0 + lerp(c[1], c[2]) * cell),
                    (x0 + lerp(c[3], c[2]) * cell, y0 + cell),
                    (x0, y0 + lerp(c[0], c[3]) * cell),
                ];
                let segs: &[(usize, usize)] = match idx {
                    1 | 14 => &[(3, 0)],
                    2 | 13 => &[(0, 1)],
                    3 | 12 => &[(3, 1)],
                    4 | 11 => &[(1, 2)],
                    6 | 9 => &[(0, 2)],
                    7 | 8 => &[(3, 2)],
                    5 => &[(3, 0), (1, 2)],
                    10 => &[(0, 1), (2, 3)],
                    _ => &[],
                };
                for &(a, b) in segs {
                    pb.lift();
                    pb.pt(e[a].0, e[a].1);
                    pb.pt(e[b].0, e[b].1);
                }
            }
        }
    }
    let mut fill = String::new();
    if rel != Rel::Eq {
        use std::fmt::Write;
        let holds = |z: f64| match rel {
            Rel::Lt | Rel::Le => z < 0.0,
            Rel::Gt | Rel::Ge => z > 0.0,
            Rel::Ne => z != 0.0,
            Rel::Eq => false,
        };
        // run-length rows of half-cells (sampled at cell centres) for a smooth enough fill
        for j in 0..ny - 1 {
            let mut run: Option<usize> = None;
            for i in 0..nx {
                let inside = i < nx - 1 && {
                    let xm = v.x0 + (i as f64 + 0.5) * cell * v.ux();
                    let ym = v.y1 - (j as f64 + 0.5) * cell * v.uy();
                    holds(g.ev(xm, ym, 0.0))
                };
                match (inside, run) {
                    (true, None) => run = Some(i),
                    (false, Some(s)) => {
                        let _ = write!(fill, "M{:.1} {:.1}h{:.1}v{:.1}h{:.1}Z", s as f64 * cell + ox, j as f64 * cell + oy, (i - s) as f64 * cell, cell, -((i - s) as f64 * cell));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
    }
    (pb.s, fill)
}

/// Roots and extrema of y = f(x) inside [xa, xb], from the samples, refined by bisection / golden section.
fn pois_fx(f: &dyn Fn(f64) -> f64, raw: &[(f64, f64)], xa: f64, xb: f64, out: &mut Vec<Value>, row: u64) {
    let mut n = 0;
    for w in raw.windows(3) {
        if n > 40 {
            break;
        }
        let (a, b, c) = (w[0], w[1], w[2]);
        if !(a.1.is_finite() && b.1.is_finite() && c.1.is_finite()) || b.0 < xa || b.0 > xb {
            continue;
        }
        // root: sign change between a and b (not a jump through a pole)
        if a.1 == 0.0 || (a.1 < 0.0) != (b.1 < 0.0) {
            let (mut lo, mut hi) = (a.0, b.0);
            let flo = f(lo);
            for _ in 0..80 {
                let m = (lo + hi) / 2.0;
                let fm = f(m);
                if !fm.is_finite() {
                    break;
                }
                if (fm < 0.0) == (flo < 0.0) {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            let x = (lo + hi) / 2.0;
            let y = f(x);
            if y.is_finite() && y.abs() < 1e-6 * (1.0 + a.1.abs().max(b.1.abs())) {
                out.push(json!({"row": row, "kind": "root", "x": clean(x), "y": 0.0}));
                n += 1;
            }
        }
        // extremum: b higher (or lower) than both neighbours
        if (b.1 > a.1 && b.1 >= c.1) || (b.1 < a.1 && b.1 <= c.1) {
            let maxi = b.1 > a.1;
            let (mut lo, mut hi) = (a.0, c.0);
            let phi = 0.618_033_988_749_895;
            for _ in 0..90 {
                let m1 = hi - phi * (hi - lo);
                let m2 = lo + phi * (hi - lo);
                let (f1, f2) = (f(m1), f(m2));
                if (f1 > f2) == maxi {
                    hi = m2;
                } else {
                    lo = m1;
                }
            }
            let x = (lo + hi) / 2.0;
            let y = f(x);
            if y.is_finite() {
                out.push(json!({"row": row, "kind": if maxi { "max" } else { "min" }, "x": clean(x), "y": clean(y)}));
                n += 1;
            }
        }
    }
    if xa <= 0.0 && xb >= 0.0 {
        let y = f(0.0);
        if y.is_finite() {
            out.push(json!({"row": row, "kind": "yint", "x": 0.0, "y": clean(y)}));
        }
    }
}

fn clean(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        return 0.0;
    }
    let r = (x * 1e9).round() / 1e9;
    if (r - x).abs() < 1e-11 * (1.0 + x.abs()) {
        r
    } else {
        x
    }
}

/// All graph rows for a view. `rows`: (id, colour index, parsed row, hidden).
pub fn plot(rows: &[(u64, u32, Ast, bool)], env: &mut Env, degrees: bool, v: View) -> Value {
    let span = v.x1 - v.x0;
    let (xa, xb) = (v.x0 - span, v.x1 + span); // sampling margin: one view each side
    let wide = View { x0: v.x0 - span, x1: v.x1 + span, y0: v.y0 - (v.y1 - v.y0), y1: v.y1 + (v.y1 - v.y0), w: v.w * 3.0, h: v.h * 3.0 };
    let mut out_rows = Vec::new();
    let mut pois = Vec::new();
    let mut explicit_fs: Vec<(u64, F)> = Vec::new();
    for (id, color, ast, hidden) in rows {
        if *hidden {
            continue;
        }
        let k = match kind_of(ast, env, degrees) {
            Ok(Some(k)) => k,
            Ok(None) => continue,
            Err(e) => {
                out_rows.push(json!({"id": id, "error": e}));
                continue;
            }
        };
        // geometry is emitted in the pixel space of `v` (the window's view); margins go outside 0..w
        let shift_x = |p: f64| p - v.w;
        let shift_y = |p: f64| p - v.h;
        let mut r = json!({"id": id, "color": color});
        match k {
            Kind::Explicit(f, rel) => {
                let fx = |x: f64| f.ev(x, 0.0, 0.0);
                let (lines, raw) = sample_fx(&fx, &wide, xa, xb, false);
                let lines: Vec<Vec<(f64, f64)>> = lines.into_iter().map(|l| l.into_iter().map(|(x, y)| (shift_x(x), shift_y(y))).collect()).collect();
                r["path"] = json!(lines_to_path_band(&lines, -1.5 * v.h, 2.5 * v.h));
                if rel != Rel::Eq {
                    // fill between the curve and the top/bottom edge of the sampled area
                    let edge = if matches!(rel, Rel::Gt | Rel::Ge) { -2.0 * v.h } else { 3.0 * v.h };
                    let mut s = String::new();
                    for l in &lines {
                        if l.len() < 2 {
                            continue;
                        }
                        use std::fmt::Write;
                        let _ = write!(s, "M{:.1} {:.1}", l[0].0, edge);
                        for &(x, y) in l {
                            let _ = write!(s, "L{:.1} {:.1}", x, y);
                        }
                        let _ = write!(s, "L{:.1} {:.1}Z", l[l.len() - 1].0, edge);
                    }
                    r["fill"] = json!(s);
                    r["dashed"] = json!(matches!(rel, Rel::Lt | Rel::Gt));
                }
                pois_fx(&fx, &raw, v.x0, v.x1, &mut pois, *id);
                explicit_fs.push((*id, f));
                // analyze(f): its asymptotes as dashed lines
                if let Ast::Call(name, a) = ast {
                    if matches!(name.as_str(), "analyze" | "investigate") {
                        if let Some(e) = crate::sym::from_ast(&a[0], degrees) {
                            let (vert, right, left) = crate::calc::asymptotes(&e, "x");
                            let mut d = String::new();
                            use std::fmt::Write;
                            for xv in vert {
                                let px = v.px(xv);
                                let _ = write!(d, "M{:.1} {:.1}L{:.1} {:.1}", px, -2.0 * v.h, px, 3.0 * v.h);
                            }
                            for kb in [right, left].into_iter().flatten() {
                                let (xa, xb) = (v.x0 - span, v.x1 + span);
                                let _ = write!(d, "M{:.1} {:.1}L{:.1} {:.1}", v.px(xa), v.py(kb.0 * xa + kb.1), v.px(xb), v.py(kb.0 * xb + kb.1));
                            }
                            if !d.is_empty() {
                                r["dash"] = json!(d);
                            }
                        }
                    }
                }
            }
            Kind::ExplicitY(g, rel) => {
                let gy = |y: f64| g.ev(0.0, y, 0.0);
                let (lines, _) = sample_fx(&gy, &wide, wide.y0, wide.y1, true);
                let lines: Vec<Vec<(f64, f64)>> = lines.into_iter().map(|l| l.into_iter().map(|(x, y)| (shift_x(x), shift_y(y))).collect()).collect();
                r["path"] = json!(lines_to_path(&lines));
                if rel != Rel::Eq {
                    let edge = if matches!(rel, Rel::Gt | Rel::Ge) { 4.0 * v.w } else { -2.0 * v.w };
                    let mut s = String::new();
                    use std::fmt::Write;
                    for l in &lines {
                        if l.len() < 2 {
                            continue;
                        }
                        let _ = write!(s, "M{:.1} {:.1}", edge, l[0].1);
                        for &(x, y) in l {
                            let _ = write!(s, "L{:.1} {:.1}", x, y);
                        }
                        let _ = write!(s, "L{:.1} {:.1}Z", edge, l[l.len() - 1].1);
                    }
                    r["fill"] = json!(s);
                    r["dashed"] = json!(matches!(rel, Rel::Lt | Rel::Gt));
                }
            }
            Kind::Polar(f) => {
                let p = |th: f64| {
                    let rr = f.ev(0.0, 0.0, th);
                    (rr * th.cos(), rr * th.sin())
                };
                let lines = sample_param(&p, &v, 0.0, std::f64::consts::TAU);
                r["path"] = json!(lines_to_path(&lines));
            }
            Kind::Param(fx, fy, t0, t1) => {
                let p = |t: f64| (fx.ev(0.0, 0.0, t), fy.ev(0.0, 0.0, t));
                let lines = sample_param(&p, &v, t0, t1);
                r["path"] = json!(lines_to_path(&lines));
            }
            Kind::Implicit(g, rel) => {
                let cell = 4.0;
                let iv = View { x0: v.x0 - span * 0.25, x1: v.x1 + span * 0.25, y0: v.y0 - (v.y1 - v.y0) * 0.25, y1: v.y1 + (v.y1 - v.y0) * 0.25, w: v.w * 1.5, h: v.h * 1.5 };
                // the grid starts a quarter view left/up of the window's view: shift into its pixel space
                let (path, fill) = implicit(&g, &iv, rel, cell, -v.w * 0.25, -v.h * 0.25);
                r["path"] = json!(path);
                // a second-order curve: foci as points, directrices/asymptotes dashed
                if rel == Rel::Eq {
                    if let Some(c) = crate::geom::conic_coefs(ast, env) {
                        let fe = crate::geom::features(&c);
                        let mut d = String::new();
                        use std::fmt::Write;
                        for (a, b, cc) in &fe.lines {
                            // a·x + b·y + c = 0 across the sampled area
                            let (xa, xb) = (v.x0 - span, v.x1 + span);
                            if b.abs() > 1e-9 {
                                let _ = write!(d, "M{:.1} {:.1}L{:.1} {:.1}", v.px(xa), v.py(-(a * xa + cc) / b), v.px(xb), v.py(-(a * xb + cc) / b));
                            } else if a.abs() > 1e-12 {
                                let xv = -cc / a;
                                let _ = write!(d, "M{:.1} {:.1}L{:.1} {:.1}", v.px(xv), -2.0 * v.h, v.px(xv), 3.0 * v.h);
                            }
                        }
                        if !d.is_empty() {
                            r["dash"] = json!(d);
                        }
                        if !fe.foci.is_empty() && fe.kind != "circle" {
                            r["points"] = json!(fe.foci.iter().map(|p| json!([v.px(p.0), v.py(p.1), clean(p.0), clean(p.1)])).collect::<Vec<_>>());
                        }
                    }
                }
                if !fill.is_empty() {
                    r["fill"] = json!(fill);
                    r["dashed"] = json!(matches!(rel, Rel::Lt | Rel::Gt));
                }
            }
            Kind::Area(f, a, b) => {
                let fx = |x: f64| f.ev(x, 0.0, 0.0);
                let (lines, _) = sample_fx(&fx, &wide, a, b, false);
                let lines: Vec<Vec<(f64, f64)>> = lines.into_iter().map(|l| l.into_iter().map(|(x, y)| (shift_x(x), shift_y(y))).collect()).collect();
                let axis = v.py(0.0);
                let mut s = String::new();
                use std::fmt::Write;
                for l in &lines {
                    if l.len() < 2 {
                        continue;
                    }
                    let _ = write!(s, "M{:.1} {:.1}", l[0].0, axis);
                    for &(x, y) in l {
                        let _ = write!(s, "L{:.1} {:.1}", x, y);
                    }
                    let _ = write!(s, "L{:.1} {:.1}Z", l[l.len() - 1].0, axis);
                }
                r["path"] = json!(lines_to_path_band(&lines, -1.5 * v.h, 2.5 * v.h));
                r["fill"] = json!(s);
                r["area"] = json!(crate::calc::simpson(&fx, a, b));
            }
            Kind::Poly(pts, closed) => {
                use std::fmt::Write;
                let mut d = String::new();
                for (i, p) in pts.iter().enumerate() {
                    let _ = write!(d, "{}{:.1} {:.1}", if i == 0 { "M" } else { "L" }, v.px(p.0), v.py(p.1));
                }
                if closed {
                    d.push('Z');
                    r["fill"] = json!(d.clone());
                    r["points"] = json!(pts.iter().map(|p| json!([v.px(p.0), v.py(p.1), clean(p.0), clean(p.1)])).collect::<Vec<_>>());
                }
                r["path"] = json!(d);
            }
            Kind::Points(pts) => {
                r["points"] = json!(pts.iter().filter(|p| p.0.is_finite() && p.1.is_finite()).map(|p| json!([v.px(p.0), v.py(p.1), clean(p.0), clean(p.1)])).collect::<Vec<_>>());
            }
        }
        out_rows.push(r);
    }
    // intersections of explicit curves (pairs), refined by bisection on f − g
    for i in 0..explicit_fs.len() {
        for j in i + 1..explicit_fs.len() {
            let (ida, fa) = &explicit_fs[i];
            let (idb, fb) = &explicit_fs[j];
            let d = |x: f64| fa.ev(x, 0.0, 0.0) - fb.ev(x, 0.0, 0.0);
            let n = 800;
            let mut prev = (v.x0, d(v.x0));
            let mut found = 0;
            for k in 1..=n {
                let x = v.x0 + (v.x1 - v.x0) * k as f64 / n as f64;
                let cur = (x, d(x));
                if prev.1.is_finite() && cur.1.is_finite() && (prev.1 < 0.0) != (cur.1 < 0.0) {
                    let (mut lo, mut hi) = (prev.0, cur.0);
                    for _ in 0..80 {
                        let m = (lo + hi) / 2.0;
                        if (d(m) < 0.0) == (d(lo) < 0.0) {
                            lo = m;
                        } else {
                            hi = m;
                        }
                    }
                    let x = (lo + hi) / 2.0;
                    let y = fa.ev(x, 0.0, 0.0);
                    if d(x).abs() < 1e-6 * (1.0 + y.abs()) && found < 20 {
                        pois.push(json!({"row": ida, "row2": idb, "kind": "cross", "x": clean(x), "y": clean(y)}));
                        found += 1;
                    }
                }
                prev = cur;
            }
        }
    }
    json!({"rows": out_rows, "pois": pois})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::collections::HashSet;

    fn rows(src: &[&str]) -> Vec<(u64, u32, Ast, bool)> {
        src.iter().enumerate().map(|(i, s)| (i as u64, 0, parse(s, &HashSet::new()).unwrap().unwrap(), false)).collect()
    }
    fn view() -> View {
        View { x0: -10.0, x1: 10.0, y0: -6.0, y1: 6.0, w: 1000.0, h: 600.0 }
    }

    #[test]
    fn curves_and_pois() {
        let mut env = Env::default();
        let t0 = std::time::Instant::now();
        let out = plot(&rows(&["y = x^2 - 4", "y = 2x + 1", "y = tan x", "x^2 + y^2 = 9", "y < sin x", "r = 2cos(3theta)", "(cos t, sin t)", "(1, 2)"]), &mut env, false, view());
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let pois = out["pois"].as_array().unwrap();
        let has = |k: &str, x: f64, y: f64| pois.iter().any(|p| p["kind"] == k && (p["x"].as_f64().unwrap() - x).abs() < 1e-6 && (p["y"].as_f64().unwrap() - y).abs() < 1e-6);
        assert!(has("root", 2.0, 0.0) && has("root", -2.0, 0.0), "{pois:?}");
        assert!(has("min", 0.0, -4.0));
        assert!(has("yint", 0.0, 1.0));
        // x² − 4 = 2x + 1 → x = 1 ± √6
        assert!(has("cross", 1.0 + 6f64.sqrt(), 2.0 * (1.0 + 6f64.sqrt()) + 1.0));
        let r = out["rows"].as_array().unwrap();
        assert_eq!(r.len(), 8);
        assert!(r.iter().all(|x| x.get("error").is_none()), "{r:?}");
        assert!(r[3]["path"].as_str().unwrap().len() > 100);
        assert!(r[4]["fill"].as_str().unwrap().len() > 100);
        // tan x: the asymptotes break the path into pieces (several "M")
        assert!(r[2]["path"].as_str().unwrap().matches('M').count() >= 5);
        eprintln!("plot of 8 rows took {ms:.1} ms (debug build)");
    }
}
