//! Calculus the way it is taught: derivatives rule by rule (sum, constant multiple, product, quotient, power +
//! chain, exponentials, the table of trig/log/inverse-trig derivatives, logarithmic differentiation), integrals by
//! linearity, the table, linear substitution, f′/f, substitution u = inner function and integration by parts, and
//! limits by substitution, cancelling the common factor, L'Hôpital's rule and leading terms at infinity.
//! Every answer is checked numerically (derivative vs. difference quotient, d/dx of the antiderivative vs. the
//! integrand, the limit vs. nearby values).

use crate::mtree::*;
use crate::steps::{step, Solution, Step};
use crate::sym::{add, depends, eval_at, mul, pow, q, qr, simplify, subst, E};
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use serde_json::Value;

fn tr(e: &E) -> Value {
    crate::mtree::sym(e)
}
fn prime(e: &E) -> Value {
    sup(par(tr(e)), t("′"))
}
fn ln(e: E) -> E {
    E::Fn("ln".into(), vec![e])
}
fn f1(name: &str, e: E) -> E {
    E::Fn(name.into(), vec![e])
}
fn neg(e: E) -> E {
    mul(vec![q(-1), e])
}

/// Split a product into (rational coefficient, factors with x, constant factors).
fn split(e: &E, x: &str) -> (BigRational, Vec<E>, Vec<E>) {
    let mut c = BigRational::one();
    let (mut var, mut cons) = (Vec::new(), Vec::new());
    let fs: Vec<E> = match e {
        E::Mul(v) => v.clone(),
        o => vec![o.clone()],
    };
    for f in fs {
        match f {
            E::Q(r) => c *= r,
            o if depends(&o, x) => var.push(o),
            o => cons.push(o),
        }
    }
    (c, var, cons)
}

// ------------------------------------------------------------------ derivatives

/// d/dx e, recording the rules used (top levels in detail, deeper ones as sub-steps).
pub fn deriv(e: &E, x: &str, st: &mut Vec<Step>, depth: usize) -> E {
    let rec = depth < 4;
    if !depends(e, x) {
        return q(0);
    }
    match e {
        E::Sym(_) => q(1),
        E::Add(terms) => {
            let mut subs = Vec::new();
            let ds: Vec<E> = terms.iter().map(|t_| deriv(t_, x, &mut subs, depth + 1)).collect();
            let r = add(ds.clone());
            if rec {
                let mut lhs = Vec::new();
                for (i, t_) in terms.iter().enumerate() {
                    if i > 0 {
                        lhs.push(op("+"));
                    }
                    lhs.push(prime(t_));
                }
                st.push(step("Sum rule: differentiate term by term, (u + v)′ = u′ + v′.", "Производная суммы: дифференцируем почленно, (u + v)′ = u′ + v′.")
                    .m(row(vec![prime(e), op("="), row(lhs), op("="), tr(&r)]))
                    .subs(subs));
            }
            r
        }
        E::Mul(_) => {
            let (c, vars, cons) = split(e, x);
            let rest = if vars.len() == 1 { vars[0].clone() } else { E::Mul(vars.clone()) };
            let k = mul([vec![E::Q(c.clone())], cons.clone()].concat());
            if !k.is_one() {
                let mut subs = Vec::new();
                let d = deriv(&rest, x, &mut subs, depth + 1);
                let r = mul(vec![k.clone(), d]);
                if rec {
                    st.push(step("Constant multiple: (c·u)′ = c·u′.", "Постоянный множитель выносим: (c·u)′ = c·u′.")
                        .m(row(vec![prime(e), op("="), tr(&k), op("·"), prime(&rest), op("="), tr(&r)]))
                        .subs(subs));
                }
                return r;
            }
            // quotient u/v when a factor is a negative power with x
            let (den, num): (Vec<E>, Vec<E>) = vars.iter().cloned().partition(|f| matches!(f, E::Pow(_, p) if matches!(&**p, E::Q(r) if r.is_negative())));
            if !den.is_empty() && !num.is_empty() {
                let u = if num.len() == 1 { num[0].clone() } else { E::Mul(num) };
                let v = simplify(&E::Mul(den.iter().map(|f| if let E::Pow(b, p) = f { pow((**b).clone(), neg((**p).clone())) } else { f.clone() }).collect()));
                let mut subs = Vec::new();
                let du = deriv(&u, x, &mut subs, depth + 1);
                let dv = deriv(&v, x, &mut subs, depth + 1);
                let r = mul(vec![add(vec![mul(vec![du.clone(), v.clone()]), neg(mul(vec![u.clone(), dv.clone()]))]), pow(v.clone(), q(-2))]);
                if rec {
                    st.push(step("Quotient rule: (u/v)′ = (u′v − uv′)/v².", "Производная частного: (u/v)′ = (u′v − uv′)/v².")
                        .m(row(vec![var("u"), op("="), tr(&u), t(",   "), var("v"), op("="), tr(&v)]))
                        .m(row(vec![frac(row(vec![par(tr(&du)), op("·"), par(tr(&v)), op("−"), par(tr(&u)), op("·"), par(tr(&dv))]), sup(par(tr(&v)), t("2"))), op("="), tr(&r)]))
                        .subs(subs));
                }
                return r;
            }
            // product rule
            let u = vars[0].clone();
            let v = if vars.len() == 2 { vars[1].clone() } else { E::Mul(vars[1..].to_vec()) };
            let mut subs = Vec::new();
            let du = deriv(&u, x, &mut subs, depth + 1);
            let dv = deriv(&v, x, &mut subs, depth + 1);
            let r = add(vec![mul(vec![du.clone(), v.clone()]), mul(vec![u.clone(), dv.clone()])]);
            if rec {
                st.push(step("Product rule: (uv)′ = u′v + uv′.", "Производная произведения: (uv)′ = u′v + uv′.")
                    .m(row(vec![var("u"), op("="), tr(&u), t(",   "), var("v"), op("="), tr(&v)]))
                    .m(row(vec![tr(&du), op("·"), par(tr(&v)), op("+"), par(tr(&u)), op("·"), par(tr(&dv)), op("="), tr(&r)]))
                    .subs(subs));
            }
            r
        }
        E::Pow(b, p) => {
            let (bx, px) = (depends(b, x), depends(p, x));
            let mut subs = Vec::new();
            if bx && !px {
                // power rule (+ chain)
                let db = deriv(b, x, &mut subs, depth + 1);
                let r = mul(vec![(**p).clone(), pow((**b).clone(), add(vec![(**p).clone(), q(-1)])), db.clone()]);
                if rec {
                    let chain = **b != E::Sym(x.into());
                    let (en, ru) = if matches!(&**p, E::Q(r) if *r == qr(1, 2).is_q().unwrap().clone()) {
                        ("Root: (√u)′ = u′/(2√u).", "Корень: (√u)′ = u′/(2√u).")
                    } else if chain {
                        ("Power rule with the chain rule: (uⁿ)′ = n·uⁿ⁻¹·u′.", "Степенная функция и правило цепочки: (uⁿ)′ = n·uⁿ⁻¹·u′.")
                    } else {
                        ("Power rule: (xⁿ)′ = n·xⁿ⁻¹.", "Производная степени: (xⁿ)′ = n·xⁿ⁻¹.")
                    };
                    let mut s = step(en, ru).m(row(vec![prime(e), op("="), tr(&r)]));
                    if chain {
                        s = s.subs(subs);
                    }
                    st.push(s);
                }
                return r;
            }
            if !bx && px {
                // a^u: (a^u)′ = a^u·ln a·u′ ; e^u: e^u·u′
                let dp = deriv(p, x, &mut subs, depth + 1);
                let lna = if **b == E::Euler { q(1) } else { ln((**b).clone()) };
                let r = mul(vec![e.clone(), lna, dp]);
                if rec {
                    let (en, ru) = if **b == E::Euler { ("Exponent: (eᵘ)′ = eᵘ·u′.", "Экспонента: (eᵘ)′ = eᵘ·u′.") } else { ("Exponential: (aᵘ)′ = aᵘ·ln a·u′.", "Показательная функция: (aᵘ)′ = aᵘ·ln a·u′.") };
                    st.push(step(en, ru).m(row(vec![prime(e), op("="), tr(&r)])).subs(subs));
                }
                return r;
            }
            // u^v: logarithmic differentiation
            let lg = mul(vec![(**p).clone(), ln((**b).clone())]);
            let dl = deriv(&lg, x, &mut subs, depth + 1);
            let r = mul(vec![e.clone(), dl]);
            if rec {
                st.push(step("Logarithmic differentiation: y = uᵛ ⇒ ln y = v·ln u ⇒ y′ = uᵛ·(v·ln u)′.", "Логарифмическое дифференцирование: y = uᵛ ⇒ ln y = v·ln u ⇒ y′ = uᵛ·(v·ln u)′.")
                    .m(row(vec![prime(e), op("="), tr(&r)]))
                    .subs(subs));
            }
            r
        }
        E::Fn(f, a) => {
            let u = a[0].clone();
            let mut subs = Vec::new();
            let du = deriv(&u, x, &mut subs, depth + 1);
            let outer = match f.as_str() {
                "sin" => f1("cos", u.clone()),
                "cos" => neg(f1("sin", u.clone())),
                "tan" => pow(f1("cos", u.clone()), q(-2)),
                "ln" => pow(u.clone(), q(-1)),
                "arcsin" => pow(add(vec![q(1), neg(pow(u.clone(), q(2)))]), qr(-1, 2)),
                "arccos" => neg(pow(add(vec![q(1), neg(pow(u.clone(), q(2)))]), qr(-1, 2))),
                "arctan" => pow(add(vec![q(1), pow(u.clone(), q(2))]), q(-1)),
                "sinh" => f1("cosh", u.clone()),
                "cosh" => f1("sinh", u.clone()),
                "abs" => mul(vec![u.clone(), pow(f1("abs", u.clone()), q(-1))]),
                _ => return E::Fn(format!("d/d{x} {f}"), vec![u]),
            };
            let r = mul(vec![outer.clone(), du.clone()]);
            if rec {
                let chain = u != E::Sym(x.into());
                let rule = match f.as_str() {
                    "sin" => "(sin u)′ = cos u", "cos" => "(cos u)′ = −sin u", "tan" => "(tg u)′ = 1/cos²u", "ln" => "(ln u)′ = 1/u",
                    "arcsin" => "(arcsin u)′ = 1/√(1 − u²)", "arccos" => "(arccos u)′ = −1/√(1 − u²)", "arctan" => "(arctg u)′ = 1/(1 + u²)",
                    _ => "",
                };
                let mut s = step(&format!("Table derivative{}: {rule}{}.", if chain { " + chain rule" } else { "" }, if chain { ", times u′" } else { "" }),
                                 &format!("Табличная производная{}: {rule}{}.", if chain { " + правило цепочки" } else { "" }, if chain { ", умножаем на u′" } else { "" }))
                    .m(row(vec![prime(e), op("="), tr(&r)]));
                if chain {
                    s = s.subs(subs);
                }
                st.push(s);
            }
            r
        }
        _ => q(0),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * (1.0 + a.abs().max(b.abs()))
}

/// Numeric check of a derivative at a few points inside the domain.
fn check_deriv(f: &E, df: &E, x: &str) -> bool {
    let mut n = 0;
    for &p in &[0.37, 1.23, -0.81, 2.7, 0.05, -1.9] {
        let (a, b) = (eval_at(f, x, p - 1e-6), eval_at(f, x, p + 1e-6));
        let d = eval_at(df, x, p);
        if !(a.is_finite() && b.is_finite() && d.is_finite()) {
            continue;
        }
        let num = (b - a) / 2e-6;
        if (num - d).abs() > 1e-4 * (1.0 + d.abs()) {
            return false;
        }
        n += 1;
    }
    n > 0
}

pub fn diff_solution(problem: Value, e: &E, x: &str) -> Solution {
    let mut st = Vec::new();
    let d = deriv(e, x, &mut st, 0);
    let ok = check_deriv(e, &d, x);
    Solution { en: "Derivative".into(), ru: "Производная".into(), problem, answer: row(vec![prime(e), op("="), tr(&d)]), steps: st, verified: ok }
}

// ------------------------------------------------------------------ integrals

/// Antiderivative with steps, or None when no taught method applies.
pub fn integ(e: &E, x: &str, st: &mut Vec<Step>, depth: usize) -> Option<E> {
    if depth > 6 {
        return None;
    }
    let xs = E::Sym(x.into());
    if !depends(e, x) {
        let r = mul(vec![e.clone(), xs.clone()]);
        if depth == 0 {
            st.push(step("Constant: ∫c dx = c·x.", "Константа: ∫c dx = c·x.").m(row(vec![integral(e, x), op("="), tr(&r)])));
        }
        return Some(r);
    }
    match e {
        E::Add(terms) => {
            let mut subs = Vec::new();
            let mut parts = Vec::new();
            for t_ in terms {
                parts.push(integ(t_, x, &mut subs, depth + 1)?);
            }
            let r = add(parts);
            st.push(step("Linearity: integrate term by term.", "Линейность: интегрируем почленно.").m(row(vec![integral(e, x), op("="), tr(&r)])).subs(subs));
            return Some(r);
        }
        E::Mul(_) => {
            let (c, vars, cons) = split(e, x);
            let k = mul([vec![E::Q(c)], cons].concat());
            if !k.is_one() {
                let rest = if vars.len() == 1 { vars[0].clone() } else { E::Mul(vars) };
                let mut subs = Vec::new();
                let r = mul(vec![k.clone(), integ(&rest, x, &mut subs, depth + 1)?]);
                st.push(step("Take the constant out: ∫c·f dx = c·∫f dx.", "Выносим константу: ∫c·f dx = c·∫f dx.").m(row(vec![integral(e, x), op("="), tr(&k), op("·"), integral(&rest, x), op("="), tr(&r)])).subs(subs));
                return Some(r);
            }
        }
        _ => {}
    }
    if let Some((r, en, ru)) = table(e, x) {
        st.push(step(en, ru).m(row(vec![integral(e, x), op("="), tr(&r)])));
        return Some(r);
    }
    // f(ax + b) with a table f: F(ax + b)/a
    if let Some(r) = linear_sub(e, x, st) {
        return Some(r);
    }
    // f′/f → ln|f|
    if let E::Mul(fs) = e {
        if let Some(r) = log_derivative(fs, x, st, e) {
            return Some(r);
        }
    }
    // u-substitution: the integrand is g(u)·u′ for an inner function u
    if let Some(r) = u_sub(e, x, st, depth) {
        return Some(r);
    }
    // integration by parts (polynomial × exp/sin/cos, ln x × polynomial)
    by_parts(e, x, st, depth)
}

fn integral(e: &E, x: &str) -> Value {
    row(vec![t("∫"), tr(e), t(" d"), var(x)])
}

/// The table, for the variable itself: (antiderivative, rule en, rule ru).
fn table(e: &E, x: &str) -> Option<(E, &'static str, &'static str)> {
    let xs = E::Sym(x.into());
    Some(match e {
        E::Sym(s) if s == x => (mul(vec![qr(1, 2), pow(xs, q(2))]), "Power rule: ∫x dx = x²/2.", "Степень: ∫x dx = x²/2."),
        E::Pow(b, p) if **b == xs && !depends(p, x) => {
            if p.is_q().is_some_and(|r| *r == -BigRational::one()) {
                (f1("ln", f1("abs", xs)), "Table: ∫dx/x = ln|x|.", "Табличный: ∫dx/x = ln|x|.")
            } else {
                let n1 = add(vec![(**p).clone(), q(1)]);
                (mul(vec![pow(xs, n1.clone()), pow(n1, q(-1))]), "Power rule: ∫xⁿ dx = xⁿ⁺¹/(n + 1), n ≠ −1.", "Степень: ∫xⁿ dx = xⁿ⁺¹/(n + 1), n ≠ −1.")
            }
        }
        E::Pow(b, p) if **p == xs && !depends(b, x) => {
            if **b == E::Euler {
                (e.clone(), "Table: ∫eˣ dx = eˣ.", "Табличный: ∫eˣ dx = eˣ.")
            } else {
                (mul(vec![e.clone(), pow(ln((**b).clone()), q(-1))]), "Table: ∫aˣ dx = aˣ/ln a.", "Табличный: ∫aˣ dx = aˣ/ln a.")
            }
        }
        E::Fn(f, a) if a[0] == xs => match f.as_str() {
            "sin" => (neg(f1("cos", xs)), "Table: ∫sin x dx = −cos x.", "Табличный: ∫sin x dx = −cos x."),
            "cos" => (f1("sin", xs), "Table: ∫cos x dx = sin x.", "Табличный: ∫cos x dx = sin x."),
            "tan" => (neg(f1("ln", f1("abs", f1("cos", xs)))), "Table: ∫tg x dx = −ln|cos x|.", "Табличный: ∫tg x dx = −ln|cos x|."),
            "ln" => (add(vec![mul(vec![xs.clone(), f1("ln", xs.clone())]), neg(xs)]), "Known: ∫ln x dx = x·ln x − x (by parts).", "Известный: ∫ln x dx = x·ln x − x (по частям)."),
            _ => return None,
        },
        // 1/cos²x, 1/sin²x
        E::Pow(b, p) if p.is_q().is_some_and(|r| *r == -BigRational::from_integer(2.into())) => match &**b {
            E::Fn(f, a) if a[0] == xs && f == "cos" => (f1("tan", xs), "Table: ∫dx/cos²x = tg x.", "Табличный: ∫dx/cos²x = tg x."),
            E::Fn(f, a) if a[0] == xs && f == "sin" => (neg(pow(f1("tan", xs), q(-1))), "Table: ∫dx/sin²x = −ctg x.", "Табличный: ∫dx/sin²x = −ctg x."),
            _ => return None,
        },
        // 1/(1 + x²), 1/√(1 − x²)
        E::Pow(b, p) => {
            let pp = crate::poly::from_ast_e(b, x)?;
            if pp.deg() == 2 && pp.coef(1).is_zero() {
                let (a2, c0) = (pp.coef(2), pp.coef(0));
                if p.is_q().is_some_and(|r| *r == -BigRational::one()) && a2.is_one() && c0.is_one() {
                    return Some((f1("arctan", xs), "Table: ∫dx/(1 + x²) = arctg x.", "Табличный: ∫dx/(1 + x²) = arctg x."));
                }
                if p.is_q().is_some_and(|r| *r == qr(-1, 2).is_q().unwrap().clone()) && a2 == -BigRational::one() && c0.is_one() {
                    return Some((f1("arcsin", xs), "Table: ∫dx/√(1 − x²) = arcsin x.", "Табличный: ∫dx/√(1 − x²) = arcsin x."));
                }
            }
            return None;
        }
        _ => return None,
    })
}

/// f(ax + b) for a table function f: the inner function is linear, so ∫ = F(ax + b)/a.
fn linear_sub(e: &E, x: &str, st: &mut Vec<Step>) -> Option<E> {
    let inner = match e {
        E::Fn(_, a) => a[0].clone(),
        E::Pow(b, p) if depends(b, x) && !depends(p, x) => (**b).clone(),
        E::Pow(b, p) if !depends(b, x) && depends(p, x) => (**p).clone(),
        _ => return None,
    };
    let lin = crate::poly::from_ast_e(&inner, x)?;
    if lin.deg() != 1 || inner == E::Sym(x.into()) {
        return None;
    }
    let a = lin.coef(1);
    let tvar = "t";
    let ts = E::Sym(tvar.into());
    let in_t = subst_expr(e, &inner, &ts);
    let (f, _, _) = table(&in_t, tvar)?;
    let r = mul(vec![E::Q(a.recip()), subst(&f, tvar, &inner)]);
    st.push(step(&format!("Linear inner function: substitute t = {}, dt = {}·dx, so ∫f(t) dt / {} = F(t)/{}.", crate::sym::plain(&inner), crate::sym::plain(&E::Q(a.clone())), crate::sym::plain(&E::Q(a.clone())), crate::sym::plain(&E::Q(a.clone()))),
                    &format!("Линейная внутренняя функция: t = {}, dt = {}·dx, значит ∫f(t) dt / {} = F(t)/{}.", crate::sym::plain(&inner), crate::sym::plain(&E::Q(a.clone())), crate::sym::plain(&E::Q(a.clone())), crate::sym::plain(&E::Q(a.clone()))))
        .m(row(vec![integral(e, x), op("="), tr(&r)])));
    Some(r)
}

/// Replace every occurrence of the sub-expression `what` by `by`.
fn subst_expr(e: &E, what: &E, by: &E) -> E {
    if e == what {
        return by.clone();
    }
    match e {
        E::Add(v) => E::Add(v.iter().map(|z| subst_expr(z, what, by)).collect()),
        E::Mul(v) => E::Mul(v.iter().map(|z| subst_expr(z, what, by)).collect()),
        E::Pow(b, p) => E::Pow(Box::new(subst_expr(b, what, by)), Box::new(subst_expr(p, what, by))),
        E::Fn(f, a) => E::Fn(f.clone(), a.iter().map(|z| subst_expr(z, what, by)).collect()),
        o => o.clone(),
    }
}

/// Are two expressions equal as functions of x (checked at several points)?
fn same_fn(a: &E, b: &E, x: &str) -> bool {
    let mut n = 0;
    for &p in &[0.31, 1.17, 2.3, -0.7, -1.45, 0.83] {
        let (u, v) = (eval_at(a, x, p), eval_at(b, x, p));
        if !(u.is_finite() && v.is_finite()) {
            continue;
        }
        if !close(u, v) {
            return false;
        }
        n += 1;
    }
    n >= 3
}

fn log_derivative(fs: &[E], x: &str, st: &mut Vec<Step>, e: &E) -> Option<E> {
    // one factor 1/h, the rest proportional to h′
    let i = fs.iter().position(|f| matches!(f, E::Pow(b, p) if depends(b, x) && p.is_q().is_some_and(|r| *r == -BigRational::one())))?;
    let E::Pow(h, _) = &fs[i] else { return None };
    let rest = simplify(&E::Mul(fs.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, f)| f.clone()).collect()));
    let mut dummy = Vec::new();
    let dh = deriv(h, x, &mut dummy, 9);
    if dh.is_zero() {
        return None;
    }
    // rest = k·h′ for a constant k
    let k = eval_at(&rest, x, 0.77) / eval_at(&dh, x, 0.77);
    if !k.is_finite() {
        return None;
    }
    let kq = BigRational::from_float(k)?.limit_denominator();
    let kq = E::Q(kq);
    if !same_fn(&rest, &mul(vec![kq.clone(), dh.clone()]), x) {
        return None;
    }
    let r = mul(vec![kq, f1("ln", f1("abs", (**h).clone()))]);
    st.push(step("The numerator is (a multiple of) the derivative of the denominator: ∫f′/f dx = ln|f|.",
                 "В числителе (с точностью до множителя) производная знаменателя: ∫f′/f dx = ln|f|.")
        .m(row(vec![integral(e, x), op("="), tr(&r)])));
    Some(r)
}

trait LimitDen {
    fn limit_denominator(self) -> BigRational;
}
impl LimitDen for BigRational {
    /// the simplest fraction within 1e-9 (continued fractions)
    fn limit_denominator(self) -> BigRational {
        let x = num_traits::ToPrimitive::to_f64(&self).unwrap_or(0.0);
        let (mut h0, mut h1, mut k0, mut k1) = (0i64, 1i64, 1i64, 0i64);
        let mut v = x;
        for _ in 0..30 {
            let a = v.floor();
            let ai = a as i64;
            let h2 = ai.saturating_mul(h1).saturating_add(h0);
            let k2 = ai.saturating_mul(k1).saturating_add(k0);
            h0 = h1;
            h1 = h2;
            k0 = k1;
            k1 = k2;
            if k1 != 0 && ((h1 as f64 / k1 as f64) - x).abs() < 1e-9 * (1.0 + x.abs()) {
                break;
            }
            let fr = v - a;
            if fr.abs() < 1e-15 {
                break;
            }
            v = 1.0 / fr;
        }
        if k1 == 0 {
            return self;
        }
        BigRational::new(h1.into(), k1.into())
    }
}

/// ∫ g(u)·u′ dx = G(u) for an inner function u of the integrand.
fn u_sub(e: &E, x: &str, st: &mut Vec<Step>, depth: usize) -> Option<E> {
    let E::Mul(fs) = e else { return None };
    let mut cands: Vec<E> = Vec::new();
    for f in fs {
        match f {
            E::Fn(_, a) if depends(&a[0], x) && a[0] != E::Sym(x.into()) => cands.push(a[0].clone()),
            E::Pow(b, p) if depends(b, x) && !depends(p, x) && **b != E::Sym(x.into()) => cands.push((**b).clone()),
            E::Pow(b, p) if !depends(b, x) && depends(p, x) && **p != E::Sym(x.into()) => cands.push((**p).clone()),
            E::Fn(_, a) if a[0] == E::Sym(x.into()) => cands.push(f.clone()), // u = sin x, ln x…
            _ => {}
        }
    }
    let tvar = "t";
    let ts = E::Sym(tvar.into());
    for u in cands {
        let mut dummy = Vec::new();
        let du = deriv(&u, x, &mut dummy, 9);
        if du.is_zero() {
            continue;
        }
        // integrand / u′ must be a function of u alone
        let g = simplify(&E::Mul(vec![e.clone(), E::Pow(Box::new(du.clone()), Box::new(q(-1)))]));
        let g_t = simplify(&subst_expr(&g, &u, &ts));
        if depends(&g_t, x) {
            continue;
        }
        let mut sub_steps = Vec::new();
        let Some(gi) = integ(&g_t, tvar, &mut sub_steps, depth + 1) else { continue };
        let r = subst(&gi, tvar, &u);
        st.push(step(&format!("Substitution t = {}: dt = ({})·dx, the integral becomes ∫ {} dt.", crate::sym::plain(&u), crate::sym::plain(&du), crate::sym::plain(&g_t)),
                     &format!("Замена t = {}: dt = ({})·dx, интеграл становится ∫ {} dt.", crate::sym::plain(&u), crate::sym::plain(&du), crate::sym::plain(&g_t)))
            .m(row(vec![integral(e, x), op("="), integral(&g_t, tvar), op("="), tr(&gi), op("="), tr(&r)]))
            .subs(sub_steps));
        return Some(r);
    }
    None
}

/// ∫u dv = uv − ∫v du with u chosen by LIATE (logarithm, then polynomial).
fn by_parts(e: &E, x: &str, st: &mut Vec<Step>, depth: usize) -> Option<E> {
    let E::Mul(fs) = e else {
        return None;
    };
    // u: ln/arctan factor if any, else the polynomial factor
    let is_poly = |f: &E| crate::poly::from_ast_e(f, x).is_some_and(|p| p.deg() >= 1);
    let ui = fs.iter().position(|f| matches!(f, E::Fn(n, _) if n == "ln" || n == "arctan" || n == "arcsin"))
        .or_else(|| fs.iter().position(is_poly))?;
    let u = fs[ui].clone();
    let dv = simplify(&E::Mul(fs.iter().enumerate().filter(|(j, _)| *j != ui).map(|(_, f)| f.clone()).collect()));
    let mut s1 = Vec::new();
    let v = integ(&dv, x, &mut s1, depth + 1)?;
    let mut d0 = Vec::new();
    let du = deriv(&u, x, &mut d0, 9);
    let vdu = simplify(&E::Mul(vec![v.clone(), du.clone()]));
    let mut s2 = Vec::new();
    let rest = integ(&vdu, x, &mut s2, depth + 1)?;
    let r = add(vec![mul(vec![u.clone(), v.clone()]), neg(rest)]);
    st.push(step("Integration by parts: ∫u dv = u·v − ∫v du (u is the factor that gets simpler when differentiated).",
                 "Интегрирование по частям: ∫u dv = u·v − ∫v du (за u берём множитель, который упрощается при дифференцировании).")
        .m(row(vec![var("u"), op("="), tr(&u), t(",  "), var("du"), op("="), tr(&du), t(" dx,   "), var("dv"), op("="), tr(&dv), t(" dx,  "), var("v"), op("="), tr(&v)]))
        .m(row(vec![integral(e, x), op("="), tr(&mul(vec![u, v])), op("−"), integral(&vdu, x), op("="), tr(&r)]))
        .subs({ let mut v = s1; v.extend(s2); v }));
    Some(r)
}

fn check_integral(f: &E, big_f: &E, x: &str) -> bool {
    let mut d = Vec::new();
    let df = deriv(big_f, x, &mut d, 9);
    same_fn(&df, f, x)
}

pub fn simpson(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let n = 2000;
    let h = (b - a) / n as f64;
    let mut s = f(a) + f(b);
    for i in 1..n {
        s += f(a + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    s * h / 3.0
}

pub fn integral_solution(problem: Value, e: &E, x: &str, bounds: Option<(E, E)>) -> Solution {
    let mut st = Vec::new();
    let anti = integ(e, x, &mut st, 0);
    match (anti, bounds) {
        (Some(big_f), None) => {
            let ok = check_integral(e, &big_f, x);
            Solution { en: "Indefinite integral".into(), ru: "Неопределённый интеграл".into(), problem, answer: row(vec![integral(e, x), op("="), tr(&big_f), op("+"), var("C")]), steps: st, verified: ok }
        }
        (Some(big_f), Some((a, b))) => {
            let fb = subst(&big_f, x, &b);
            let fa = subst(&big_f, x, &a);
            let val = add(vec![fb.clone(), neg(fa.clone())]);
            let num = simpson(&|z| eval_at(e, x, z), a.to_f64(), b.to_f64());
            st.push(step("Newton–Leibniz: ∫ₐᵇ f dx = F(b) − F(a).", "Формула Ньютона–Лейбница: ∫ₐᵇ f dx = F(b) − F(a).")
                .m(row(vec![tr(&fb), op("−"), par(tr(&fa)), op("="), tr(&val)])));
            let ok = check_integral(e, &big_f, x) && close(val.to_f64(), num);
            let ans = if val.has_sym() {
                t(&crate::num::fmt_float(num, 10))
            } else if val.is_q().is_some_and(|r| r.is_integer()) {
                tr(&val)
            } else {
                row(vec![tr(&val), t(&format!("  ≈ {}", crate::num::fmt_float(val.to_f64(), 8)))])
            };
            Solution { en: "Definite integral".into(), ru: "Определённый интеграл".into(), problem, answer: ans, steps: st, verified: ok }
        }
        (None, Some((a, b))) => {
            let num = simpson(&|z| eval_at(e, x, z), a.to_f64(), b.to_f64());
            st.clear();
            st.push(step("No elementary antiderivative found by the taught methods: the value is computed numerically (Simpson's rule).",
                         "Первообразная изучаемыми методами не находится: значение вычислено численно (формула Симпсона)."));
            Solution { en: "Definite integral".into(), ru: "Определённый интеграл".into(), problem, answer: t(&format!("≈ {}", crate::num::fmt_float(num, 10))), steps: st, verified: num.is_finite() }
        }
        (None, None) => Solution {
            en: "Indefinite integral".into(),
            ru: "Неопределённый интеграл".into(),
            problem,
            answer: t("—"),
            steps: vec![step("None of the taught methods (table, linearity, substitution, by parts) applies to this integrand yet.",
                             "Изучаемые методы (таблица, линейность, замена, по частям) к этому интегралу пока не применимы.")],
            verified: false,
        },
    }
}

// ------------------------------------------------------------------ limits

pub fn limit_solution(problem: Value, e: &E, x: &str, a: &E, at_inf: i32) -> Solution {
    let mut st = Vec::new();
    let near = |h: f64| -> f64 {
        if at_inf != 0 {
            eval_at(e, x, at_inf as f64 * h.recip())
        } else {
            eval_at(e, x, a.to_f64() + h)
        }
    };
    // numeric estimate from both sides (one side at infinity)
    let est = {
        let hs = [1e-4, 1e-5, 1e-6];
        let l: Vec<f64> = hs.iter().map(|&h| near(h)).collect();
        let r: Vec<f64> = hs.iter().map(|&h| near(-h)).collect();
        if at_inf != 0 { l[2] } else if close(l[2], r[2]) { (l[2] + r[2]) / 2.0 } else { f64::NAN }
    };
    let res: Option<E> = if at_inf == 0 {
        let direct = subst(e, x, a);
        let dv = direct.to_f64();
        // a denominator that vanishes makes direct substitution meaningless (0·(1/0) must not become 0)
        let den_zero = subst(&num_den(e).1, x, a).is_zero();
        if !den_zero && dv.is_finite() && !direct.has_sym() {
            st.push(step("Direct substitution: the function is continuous there.", "Непосредственная подстановка: функция там непрерывна.")
                .m(row(vec![tr(e), op("→"), tr(&direct)])));
            Some(direct)
        } else {
            limit_indeterminate(e, x, a, &mut st, 0)
        }
    } else {
        limit_infinity(e, x, at_inf, &mut st)
    };
    let (answer, ok) = match &res {
        Some(E::Fn(name, args)) if args.is_empty() && name.contains('∞') => (tr(res.as_ref().unwrap()), !est.is_finite() || est.abs() > 1e5),
        Some(r) => {
            let v = r.to_f64();
            (tr(r), close(v, est))
        }
        None => {
            st.push(step("Estimated numerically from both sides.", "Оценено численно с двух сторон."));
            (t(&if est.is_finite() { crate::num::fmt_float(est, 8) } else { "does not exist / ∞".into() }), est.is_finite())
        }
    };
    Solution { en: "Limit".into(), ru: "Предел".into(), problem, answer, steps: st, verified: ok }
}

fn num_den(e: &E) -> (E, E) {
    match e {
        E::Mul(fs) => {
            let (d, n): (Vec<E>, Vec<E>) = fs.iter().cloned().partition(|f| matches!(f, E::Pow(_, p) if matches!(&**p, E::Q(r) if r.is_negative())));
            let den = simplify(&E::Mul(d.iter().map(|f| if let E::Pow(b, p) = f { pow((**b).clone(), neg((**p).clone())) } else { f.clone() }).collect()));
            (simplify(&E::Mul(n)), den)
        }
        E::Pow(b, p) if matches!(&**p, E::Q(r) if r.is_negative()) => (q(1), pow((**b).clone(), neg((**p).clone()))),
        o => (o.clone(), q(1)),
    }
}

fn limit_indeterminate(e: &E, x: &str, a: &E, st: &mut Vec<Step>, depth: usize) -> Option<E> {
    if depth > 3 {
        return None;
    }
    let (n, d) = num_den(e);
    let (nv, dv) = (subst(&n, x, a).to_f64(), subst(&d, x, a).to_f64());
    if !(nv.abs() < 1e-12 && dv.abs() < 1e-12) {
        if dv.abs() < 1e-12 && nv.abs() > 1e-12 {
            st.push(step("The denominator → 0 while the numerator doesn't: the limit is infinite (check the sign on each side).",
                         "Знаменатель → 0, числитель — нет: предел бесконечен (знак проверяем с каждой стороны)."));
            return Some(E::Fn("∞".into(), vec![]));
        }
        return None;
    }
    // 0/0: polynomials → cancel (x − a); otherwise L'Hôpital
    if let (Some(pn), Some(pd)) = (crate::poly::from_ast_e(&n, x), crate::poly::from_ast_e(&d, x)) {
        if let Some(ar) = a.is_q() {
            let (mut pn, mut pd) = (pn, pd);
            let mut k = 0;
            while pn.eval(ar).is_zero() && pd.eval(ar).is_zero() && pn.deg() >= 1 && pd.deg() >= 1 {
                pn = pn.div_linear(ar).0;
                pd = pd.div_linear(ar).0;
                k += 1;
            }
            let xa = crate::algebra::poly_tree(&crate::poly::Poly { c: vec![-ar.clone(), BigRational::one()] }, x);
            let (vn, vd) = (pn.eval(ar), pd.eval(ar));
            if vd.is_zero() {
                return None;
            }
            let r = E::Q(vn.clone() / vd.clone());
            st.push(step(&format!("0/0: both numerator and denominator vanish at {x} = {}; factor out ({x} − {}){} and cancel.", crate::sym::plain(a), crate::sym::plain(a), if k > 1 { format!("^{k}") } else { String::new() }),
                         &format!("0/0: числитель и знаменатель обращаются в 0 при {x} = {}; выделяем множитель ({x} − {}){} и сокращаем.", crate::sym::plain(a), crate::sym::plain(a), if k > 1 { format!("^{k}") } else { String::new() }))
                .m(row(vec![frac(row(vec![par(xa.clone()), op("·"), par(crate::algebra::poly_tree(&pn, x))]), row(vec![par(xa), op("·"), par(crate::algebra::poly_tree(&pd, x))])), op("="),
                            frac(crate::algebra::poly_tree(&pn, x), crate::algebra::poly_tree(&pd, x)), op("→"), frac(rat(&vn), rat(&vd)), op("="), tr(&r)])));
            return Some(r);
        }
    }
    let mut d1 = Vec::new();
    let dn = deriv(&n, x, &mut d1, 9);
    let dd = deriv(&d, x, &mut d1, 9);
    let ratio = simplify(&E::Mul(vec![dn.clone(), E::Pow(Box::new(dd.clone()), Box::new(q(-1)))]));
    let sv = subst(&ratio, x, a);
    st.push(step("0/0: L'Hôpital's rule — the limit of f/g equals the limit of f′/g′.", "0/0: правило Лопиталя — предел f/g равен пределу f′/g′.")
        .m(row(vec![frac(prime(&n), prime(&d)), op("="), frac(tr(&dn), tr(&dd))])));
    if sv.to_f64().is_finite() && !sv.has_sym() {
        st.push(step("Now substitute:", "Теперь подставляем:").m(row(vec![op("→"), tr(&sv)])));
        return Some(sv);
    }
    limit_indeterminate(&ratio, x, a, st, depth + 1)
}

fn limit_infinity(e: &E, x: &str, sign: i32, st: &mut Vec<Step>) -> Option<E> {
    let (n, d) = num_den(e);
    let (pn, pd) = (crate::poly::from_ast_e(&n, x)?, crate::poly::from_ast_e(&d, x)?);
    let (dn, dd) = (pn.deg(), pd.deg());
    let r = if dn < dd {
        q(0)
    } else if dn == dd {
        E::Q(pn.lead() / pd.lead())
    } else {
        let s = pn.lead() / pd.lead();
        let pos = s.is_positive() == (sign > 0 || (dn - dd) % 2 == 0);
        E::Fn(if pos { "+∞" } else { "−∞" }.into(), vec![])
    };
    st.push(step("At infinity a polynomial behaves like its leading term: compare the highest powers of numerator and denominator.",
                 "На бесконечности многочлен ведёт себя как старший член: сравниваем старшие степени числителя и знаменателя.")
        .m(row(vec![frac(row(vec![rat(&pn.lead()), sup(var(x), t(&dn.to_string()))]), row(vec![rat(&pd.lead()), sup(var(x), t(&dd.to_string()))])), op("→"), tr(&r)])));
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::collections::HashSet;

    fn e(src: &str) -> E {
        crate::sym::from_ast(&parse(src, &HashSet::new()).unwrap().unwrap(), false).unwrap()
    }

    #[test]
    fn derivatives() {
        for f in ["x^3 - 2x + 1", "sin(x)^2", "x*e^x", "ln(x^2 + 1)", "(x+1)/(x-1)", "sqrt(x^2+1)", "x^x", "arctan(2x)", "e^(sin x)", "cos(3x)*x^2"] {
            let s = diff_solution(t(""), &e(f), "x");
            assert!(s.verified, "d/dx {f}");
        }
    }

    #[test]
    fn integrals() {
        for f in ["3x^2 + 2x - 1", "cos x", "e^(2x)", "1/x", "x*e^x", "x*sin x", "ln x", "2x/(x^2+1)", "sin(3x+1)", "x*cos(x^2)", "1/(1+x^2)", "x^2*e^x", "e^x/(e^x + 1)"] {
            let s = integral_solution(t(""), &e(f), "x", None);
            assert!(s.verified, "∫ {f}");
        }
        let s = integral_solution(t(""), &e("x^2"), "x", Some((q(0), q(3))));
        assert!(s.verified);
        assert!(crate::mtree::flat(&s.answer).starts_with('9'));
    }

    #[test]
    fn investigation() {
        let s = analyze_solution(t(""), &e("x^3 - 3x"), "x");
        assert!(s.verified);
        let a = crate::mtree::flat(&s.answer);
        assert!(a.contains("max (−1; 2)") && a.contains("min (1; −2)"), "{a}");
        let (v, r, _) = asymptotes(&e("(x^2 + 1)/(x - 1)"), "x");
        assert_eq!(v, vec![1.0]);
        assert_eq!(r, Some((1.0, 1.0)));
    }

    #[test]
    fn limits() {
        let l = |f: &str, a: E| limit_solution(t(""), &e(f), "x", &a, 0);
        assert!(l("(x^2 - 1)/(x - 1)", q(1)).verified);
        assert_eq!(crate::mtree::flat(&l("(x^2 - 1)/(x - 1)", q(1)).answer), "2");
        assert!(l("sin(x)/x", q(0)).verified);
        assert!(l("(e^x - 1)/x", q(0)).verified);
        let s = limit_solution(t(""), &e("(3x^2 + 1)/(x^2 - 5)"), "x", &q(0), 1);
        assert_eq!(crate::mtree::flat(&s.answer), "3");
    }
}

/// diff(f[, x]) · integrate(f[, a, b]) · lim(f, x, a): the solution, if the row is one of these.
pub fn solve_call(a: &crate::parse::Ast, degrees: bool) -> Option<Result<Solution, String>> {
    use crate::parse::Ast;
    let Ast::Call(f, args) = a else { return None };
    let kind = match f.as_str() {
        "diff" | "derivative" => 0,
        "integrate" | "int" => 1,
        "lim" | "limit" => 2,
        "analyze" | "investigate" => 3,
        _ => return None,
    };
    let problem = crate::mtree::ast(a);
    let conv = |x: &Ast| crate::sym::from_ast(x, degrees).ok_or_else(|| "this expression can't be handled symbolically yet".to_string());
    let var_of = |x: &Ast| -> Option<String> { if let Ast::Var(v) = x { Some(v.clone()) } else { None } };
    Some((|| {
        let body = conv(args.first().ok_or("missing the function")?)?;
        match kind {
            0 => {
                let x = args.get(1).and_then(var_of).unwrap_or_else(|| "x".into());
                Ok(diff_solution(problem, &body, &x))
            }
            3 => Ok(analyze_solution(problem, &body, "x")),
            1 => match args.len() {
                1 => Ok(integral_solution(problem, &body, "x", None)),
                2 => Ok(integral_solution(problem, &body, &var_of(&args[1]).ok_or("integrate(f, x) or integrate(f, a, b)")?, None)),
                3 => Ok(integral_solution(problem, &body, "x", Some((conv(&args[1])?, conv(&args[2])?)))),
                _ => Err("integrate(f), integrate(f, a, b)".into()),
            },
            _ => {
                let (x, target) = match args.len() {
                    2 => ("x".to_string(), &args[1]),
                    3 => (var_of(&args[1]).ok_or("lim(f, x, a)")?, &args[2]),
                    _ => return Err("lim(f, x, a)".into()),
                };
                let inf = match target {
                    Ast::Var(v) if v == "∞" => 1,
                    Ast::Neg(z) if matches!(&**z, Ast::Var(v) if v == "∞") => -1,
                    _ => 0,
                };
                let at = if inf == 0 { conv(target)? } else { q(0) };
                Ok(limit_solution(problem, &body, &x, &at, inf))
            }
        }
    })())
}

// ------------------------------------------------------------------ function investigation (исследование функции)

/// Real zeros of f on [−l, l] (sign changes refined by bisection; poles rejected), sorted.
pub fn zeros(f: &dyn Fn(f64) -> f64, l: f64) -> Vec<f64> {
    let n = 4000;
    let mut out: Vec<f64> = Vec::new();
    let mut prev = (-l, f(-l));
    for i in 1..=n {
        let x = -l + 2.0 * l * i as f64 / n as f64;
        let cur = (x, f(x));
        if cur.1 == 0.0 {
            out.push(x);
        } else if prev.1.is_finite() && cur.1.is_finite() && (prev.1 < 0.0) != (cur.1 < 0.0) && prev.1 != 0.0 {
            let (mut a, mut b) = (prev.0, cur.0);
            for _ in 0..100 {
                let m = (a + b) / 2.0;
                if (f(m) < 0.0) == (f(a) < 0.0) {
                    a = m;
                } else {
                    b = m;
                }
            }
            let r = (a + b) / 2.0;
            if f(r).abs() < 1e-6 * (1.0 + prev.1.abs().min(cur.1.abs())) {
                out.push(r);
            }
        }
        prev = cur;
    }
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    out.iter().map(|&z| if z.abs() < 1e-12 { 0.0 } else { z }).collect()
}

/// Points where f is undefined between defined neighbours (poles / holes), on [−l, l].
fn breaks(f: &dyn Fn(f64) -> f64, l: f64) -> Vec<f64> {
    // a pole: the sign flips between neighbours that are both larger in size than the values further out
    // (near a zero crossing it's the other way round); an undefined point between defined neighbours is a hole
    let n = 4000;
    let h = 2.0 * l / n as f64;
    let mut out: Vec<f64> = Vec::new();
    for i in 4..n - 4 {
        let x = -l + h * i as f64;
        let (a, b) = (f(x), f(x + h));
        let pole = a.is_finite() && b.is_finite() && (a < 0.0) != (b < 0.0) && {
            let (fa, fb) = (f(x - 3.0 * h).abs(), f(x + 4.0 * h).abs());
            fa.is_finite() && fb.is_finite() && a.abs().min(b.abs()) > fa.max(fb)
        };
        let hole = a.is_finite() && b.is_finite() && !f(x + h / 2.0).is_finite();
        if pole || hole {
            let (mut lo, mut hi) = (x, x + h);
            for _ in 0..80 {
                let m = (lo + hi) / 2.0;
                let fm = f(m);
                if !fm.is_finite() {
                    lo = m;
                    hi = m;
                    break;
                }
                if (fm < 0.0) == (a < 0.0) {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            let r = (lo + hi) / 2.0;
            let r = if (r - r.round()).abs() < 1e-6 { r.round() } else { r };
            if !out.iter().any(|z| (z - r).abs() < 1e-5) {
                out.push(r);
            }
        }
    }
    out
}

/// (vertical asymptotes, slant/horizontal asymptote at +∞ as (k, b), at −∞).
pub fn asymptotes(e: &E, x: &str) -> (Vec<f64>, Option<(f64, f64)>, Option<(f64, f64)>) {
    let f = |z: f64| eval_at(e, x, z);
    let vert: Vec<f64> = breaks(&f, 30.0).into_iter().filter(|&p| {
        let (a, b) = (f(p - 1e-7).abs(), f(p + 1e-7).abs());
        a > 1e5 || b > 1e5
    }).collect();
    let slant = |s: f64| -> Option<(f64, f64)> {
        // slope from a difference quotient far out (f(x)/x would carry a 1/x error), checked further out
        let (x1, x2, x3) = (s * 1e6, s * 2e6, s * 4e6);
        let (f1_, f2_, f3_) = (f(x1), f(x2), f(x3));
        if !(f1_.is_finite() && f2_.is_finite() && f3_.is_finite()) {
            return None;
        }
        let k = (f2_ - f1_) / (x2 - x1);
        let k2 = (f3_ - f2_) / (x3 - x2);
        if (k - k2).abs() > 1e-6 * (1.0 + k.abs()) {
            return None;
        }
        let k = if k.abs() < 1e-7 { 0.0 } else { (k * 1e6).round() / 1e6 };
        let (b2, b3) = (f2_ - k * x2, f3_ - k * x3);
        if (b2 - b3).abs() > 1e-3 * (1.0 + b2.abs()) {
            return None;
        }
        Some((k, (b3 * 1e5).round() / 1e5))
    };
    (vert, slant(1.0), slant(-1.0))
}

pub fn analyze_solution(problem: Value, e: &E, x: &str) -> Solution {
    let f = |z: f64| eval_at(e, x, z);
    let mut st = Vec::new();
    let num = |v: f64| t(&crate::num::fmt_float(v, 6));
    // 1. domain
    let excluded: Vec<f64> = breaks(&f, 30.0);
    let neg_side = (-3000..=3000).map(|i| i as f64 / 100.0).filter(|&z| !f(z).is_finite()).count();
    let mut dom = vec![t("D(f): ")];
    if excluded.is_empty() && neg_side == 0 {
        dom.push(t("ℝ"));
    } else if neg_side > excluded.len() * 3 {
        // undefined on whole stretches (roots, logarithms): show where it is defined in the scanned range
        let mut segs: Vec<(f64, f64)> = Vec::new();
        let mut start: Option<f64> = None;
        let n = 6000;
        for i in 0..=n {
            let z = -30.0 + 60.0 * i as f64 / n as f64;
            let ok = f(z).is_finite();
            match (ok, start) {
                (true, None) => start = Some(z),
                (false, Some(s)) => {
                    segs.push((s, z));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            segs.push((s, f64::INFINITY));
        }
        for (i, (a, b)) in segs.iter().enumerate() {
            if i > 0 {
                dom.push(op("∪"));
            }
            let lo = if *a <= -29.99 { "−∞".to_string() } else { crate::num::fmt_float(*a, 4) };
            let hi = if b.is_infinite() || *b >= 29.99 { "+∞".to_string() } else { crate::num::fmt_float(*b, 4) };
            dom.push(t(&format!("[{lo}; {hi})").replace("[−∞", "(−∞")));
        }
    } else {
        dom.push(t("ℝ \\ {"));
        for (i, p) in excluded.iter().enumerate() {
            if i > 0 {
                dom.push(t("; "));
            }
            dom.push(num(*p));
        }
        dom.push(t("}"));
    }
    st.push(step("Domain: where the formula makes sense (no division by zero, logarithms of positive numbers, even roots of non-negative ones).",
                 "Область определения: где формула имеет смысл (нет деления на ноль, логарифм от положительного, корень чётной степени от неотрицательного).")
        .m(row(dom)));
    // 2. parity / periodicity
    let fneg = subst(e, x, &neg(E::Sym(x.into())));
    let (en, ru, m) = if same_fn(&fneg, e, x) {
        ("Even: f(−x) = f(x), the graph is symmetric about the y-axis.", "Чётная: f(−x) = f(x), график симметричен относительно оси Oy.", row(vec![t("f(−x) = f(x)")]))
    } else if same_fn(&fneg, &neg(e.clone()), x) {
        ("Odd: f(−x) = −f(x), the graph is symmetric about the origin.", "Нечётная: f(−x) = −f(x), график симметричен относительно начала координат.", row(vec![t("f(−x) = −f(x)")]))
    } else {
        ("Neither even nor odd.", "Функция общего вида (ни чётная, ни нечётная).", row(vec![t("f(−x) = "), tr(&fneg)]))
    };
    st.push(step(en, ru).m(m));
    let tau = std::f64::consts::TAU;
    if (0..20).all(|i| {
        let z = -3.0 + i as f64 * 0.37;
        let (a, b) = (f(z), f(z + tau));
        !a.is_finite() || !b.is_finite() || close(a, b)
    }) && !crate::poly::from_ast_e(e, x).is_some() && e_has_trig(e)
    {
        let half = (0..20).all(|i| {
            let z = -3.0 + i as f64 * 0.37;
            let (a, b) = (f(z), f(z + tau / 2.0));
            !a.is_finite() || !b.is_finite() || close(a, b)
        });
        st.push(step(if half { "Periodic with period π." } else { "Periodic with period 2π." }, if half { "Периодическая, период π." } else { "Периодическая, период 2π." }));
    }
    // 3. intercepts
    let zs = zeros(&f, 30.0);
    let mut line = vec![t("f(x) = 0:  ")];
    if zs.is_empty() {
        line.push(t("—"));
    }
    for (i, z) in zs.iter().take(12).enumerate() {
        if i > 0 {
            line.push(t(";  "));
        }
        line.push(num(*z));
    }
    let f0 = f(0.0);
    if f0.is_finite() {
        line.push(t(&format!("     f(0) = {}", crate::num::fmt_float(f0, 6))));
    }
    st.push(step("Intercepts: zeros of the function (the x-axis) and f(0) (the y-axis).", "Точки пересечения с осями: нули функции (ось Ox) и f(0) (ось Oy).").m(row(line)));
    // 4. asymptotes
    let (vert, right, left) = asymptotes(e, x);
    let mut asy = Vec::new();
    for v in &vert {
        asy.push(row(vec![t("x = "), num(*v), t(if crate::num::fmt_float(*v, 6).len() < 8 { "   (vertical)" } else { "" })]));
    }
    let slant_tree = |kb: (f64, f64), side: &str| -> Value {
        if kb.0 == 0.0 {
            row(vec![t(&format!("y = {}   (horizontal, x → {side})", crate::num::fmt_float(kb.1, 6)))])
        } else {
            row(vec![t(&format!("y = {}·x {} {}   (slant, x → {side})", crate::num::fmt_float(kb.0, 6), if kb.1 < 0.0 { "−" } else { "+" }, crate::num::fmt_float(kb.1.abs(), 6)))])
        }
    };
    match (right, left) {
        (Some(r), Some(l)) if close(r.0, l.0) && close(r.1, l.1) => asy.push(slant_tree(r, "±∞")),
        (r, l) => {
            if let Some(r) = r {
                asy.push(slant_tree(r, "+∞"));
            }
            if let Some(l) = l {
                asy.push(slant_tree(l, "−∞"));
            }
        }
    }
    let mut s4 = step("Asymptotes: vertical where f → ∞ at a finite point; y = kx + b with k = lim f(x)/x, b = lim (f(x) − kx) at infinity.",
                      "Асимптоты: вертикальные — где f → ∞ в конечной точке; наклонные y = kx + b, k = lim f(x)/x, b = lim (f(x) − kx) на бесконечности.");
    if asy.is_empty() {
        s4 = s4.m(t("—"));
    }
    for a in asy {
        s4 = s4.m(a);
    }
    st.push(s4);
    // 5. monotonicity and extrema from f′
    let mut dsteps = Vec::new();
    let d1 = deriv(e, x, &mut dsteps, 0);
    let fd = |z: f64| eval_at(&d1, x, z);
    let crit = zeros(&fd, 30.0);
    let chart = |pts: &[f64], g: &dyn Fn(f64) -> f64, up: &str, down: &str| -> (Value, Vec<(f64, f64, bool)>) {
        let mut edges = vec![f64::NEG_INFINITY];
        let mut all: Vec<f64> = pts.iter().chain(excluded.iter()).cloned().collect();
        all.sort_by(f64::total_cmp);
        all.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        edges.extend(all);
        edges.push(f64::INFINITY);
        let mut c = Vec::new();
        let mut iv = Vec::new();
        for w in edges.windows(2) {
            let probe = if w[0].is_infinite() && w[1].is_infinite() { 0.5 } else if w[0].is_infinite() { w[1] - 1.0 } else if w[1].is_infinite() { w[0] + 1.0 } else { (w[0] + w[1]) / 2.0 };
            let v = g(probe);
            if !v.is_finite() {
                continue;
            }
            let pos = v > 0.0;
            if !c.is_empty() {
                c.push(t("   "));
            }
            let b = |z: f64| if z == f64::NEG_INFINITY { "−∞".to_string() } else if z == f64::INFINITY { "+∞".to_string() } else { crate::num::fmt_float(z, 5) };
            c.push(t(&format!("({}; {}): {} {}", b(w[0]), b(w[1]), if pos { "+" } else { "−" }, if pos { up } else { down }).replace('-', "−")));
            iv.push((w[0], w[1], pos));
        }
        (row(c), iv)
    };
    let (mono, iv) = chart(&crit, &fd, "↗", "↘");
    let mut ext = Vec::new();
    for w in iv.windows(2) {
        let p = w[0].1;
        if !p.is_finite() || excluded.iter().any(|z| (z - p).abs() < 1e-9) || w[0].2 == w[1].2 {
            continue;
        }
        let kind = if w[0].2 { "max" } else { "min" };
        if !ext.is_empty() {
            ext.push(t(",   "));
        }
        ext.push(t(&format!("{kind} ({}; {})", crate::num::fmt_float(p, 5), crate::num::fmt_float(f(p), 5)).replace('-', "−")));
    }
    let mut s5 = step("Monotonicity: f increases where f′ > 0 and decreases where f′ < 0; where f′ changes sign there is an extremum.",
                      "Монотонность: f возрастает при f′ > 0 и убывает при f′ < 0; где f′ меняет знак — экстремум.")
        .m(row(vec![t("f′(x) = "), tr(&d1)]))
        .m(mono)
        .subs(dsteps);
    if !ext.is_empty() {
        s5 = s5.m(row(ext.clone()));
    }
    st.push(s5);
    // 6. convexity and inflection from f″
    let mut d2steps = Vec::new();
    let d2 = deriv(&d1, x, &mut d2steps, 2);
    let fdd = |z: f64| eval_at(&d2, x, z);
    let infl = zeros(&fdd, 30.0);
    let (conv, iv2) = chart(&infl, &fdd, "∪", "∩");
    let mut ip = Vec::new();
    for w in iv2.windows(2) {
        let p = w[0].1;
        if !p.is_finite() || excluded.iter().any(|z| (z - p).abs() < 1e-9) || w[0].2 == w[1].2 {
            continue;
        }
        ip.push(t(&format!("({}; {})   ", crate::num::fmt_float(p, 6), crate::num::fmt_float(f(p), 6))));
    }
    let mut s6 = step("Convexity: f″ > 0 — convex (∪), f″ < 0 — concave (∩); a sign change of f″ is an inflection point.",
                      "Выпуклость: f″ > 0 — выпукла вниз (∪), f″ < 0 — выпукла вверх (∩); смена знака f″ — точка перегиба.")
        .m(row(vec![t("f″(x) = "), tr(&d2)]))
        .m(conv);
    if !ip.is_empty() {
        s6 = s6.m(row([vec![t("inflection: ")], ip].concat()));
    }
    st.push(s6);
    st.push(step("Put it together on the graph (the curve, its asymptotes dashed; click the grey dots for zeros and extrema).",
                 "Собираем всё на графике (кривая, асимптоты пунктиром; серые точки — нули и экстремумы)."));
    let ok = check_deriv(e, &d1, x);
    let answer = if ext.is_empty() { t("no extrema") } else { row(ext) };
    Solution { en: "Function investigation".into(), ru: "Исследование функции".into(), problem, answer, steps: st, verified: ok }
}

fn e_has_trig(e: &E) -> bool {
    match e {
        E::Fn(f, a) => matches!(f.as_str(), "sin" | "cos" | "tan") || a.iter().any(e_has_trig),
        E::Add(v) | E::Mul(v) => v.iter().any(e_has_trig),
        E::Pow(b, p) => e_has_trig(b) || e_has_trig(p),
        _ => false,
    }
}
