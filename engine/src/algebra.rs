//! Equations and inequalities in one unknown, solved the way a textbook does: everything to one side, clear
//! fractions, then by degree — linear (isolate x), quadratic (x² = k, x(ax + b) = 0, or the discriminant),
//! biquadratic (t = x²), higher degree (take xᵏ out, rational-root theorem + synthetic division, down to a
//! quadratic). Inequalities by the interval method (signs between the roots). Also expand(…) and factor(…).

use crate::eval::{Env, Val};
use crate::mtree::*;
use crate::parse::{Ast, Rel};
use crate::poly::{from_ast, q, Poly};
use crate::steps::{step, Solution, Step};
use crate::sym::{self, E};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde_json::{json, Value};

/// The one unknown letter of an expression (not defined in env), if there is exactly one.
fn unknown(a: &Ast, env: &Env, found: &mut Vec<String>) {
    match a {
        Ast::Var(n) => {
            let known = env.vars.contains_key(n) || matches!(n.as_str(), "pi" | "e" | "∞" | "tau");
            if !known && !found.contains(n) {
                found.push(n.clone());
            }
        }
        Ast::Num(_) => {}
        Ast::Neg(x) | Ast::Fact(x) | Ast::Deg(x) | Ast::Abs(x) => unknown(x, env, found),
        Ast::Add(x, y) | Ast::Sub(x, y) | Ast::Mul(x, y, _) | Ast::Div(x, y) | Ast::Pow(x, y) => {
            unknown(x, env, found);
            unknown(y, env, found);
        }
        Ast::Call(_, v) | Ast::List(v) | Ast::Tuple(v) => v.iter().for_each(|x| unknown(x, env, found)),
        Ast::Rel(f, ch) => {
            unknown(f, env, found);
            ch.iter().for_each(|(_, e)| unknown(e, env, found));
        }
        _ => found.push("?".into()),
    }
}

fn consts<'a>(env: &'a Env) -> impl Fn(&str) -> Option<BigRational> + 'a {
    move |n: &str| match env.vars.get(n) {
        Some(Val::N(crate::num::Num::Q(r))) => Some(r.clone()),
        _ => None,
    }
}

/// `lhs (rel) rhs` with one unknown that is polynomial on both sides → (var, rel, lhs − rhs, was it already "… = 0").
pub fn detect(a: &Ast, env: &Env) -> Option<(String, Rel, Poly, bool)> {
    let Ast::Rel(l, chain) = a else { return None };
    if chain.len() != 1 {
        return None;
    }
    let (rel, r) = (chain[0].0, &chain[0].1);
    let mut vars = Vec::new();
    unknown(a, env, &mut vars);
    if vars.len() != 1 || vars[0] == "y" || vars[0] == "?" {
        return None;
    }
    let v = vars[0].clone();
    // a definition like "a = 3" is not an equation
    if matches!(&**l, Ast::Var(n) if *n == v) && !crate::plot::ast_uses(r, &v) {
        return None;
    }
    let c = consts(env);
    let p = from_ast(l, &v, &c)?.sub(&from_ast(r, &v, &c)?);
    let zero_rhs = matches!(r, Ast::Num(s) if s == "0");
    Some((v, rel, p, zero_rhs))
}

// ------------------------------------------------------------------ display

pub fn poly_tree(p: &Poly, v: &str) -> Value {
    if p.c.is_empty() {
        return t("0");
    }
    let mut c = Vec::new();
    for i in (0..p.c.len()).rev() {
        let a = &p.c[i];
        if a.is_zero() {
            continue;
        }
        let neg = a.is_negative();
        let mag = a.abs();
        if c.is_empty() {
            if neg {
                c.push(t("−"));
            }
        } else {
            c.push(op(if neg { "−" } else { "+" }));
        }
        let xpart = match i {
            0 => None,
            1 => Some(var(v)),
            _ => Some(sup(var(v), t(&i.to_string()))),
        };
        match xpart {
            None => c.push(rat(&mag)),
            Some(x) => {
                if !mag.is_one() {
                    c.push(rat(&mag));
                }
                c.push(x);
            }
        }
    }
    row(c)
}

fn rel_tree(p: &Poly, v: &str, r: Rel) -> Value {
    row(vec![poly_tree(p, v), op(r.sym()), t("0")])
}

fn e_tree(e: &E) -> Value {
    crate::mtree::sym(e)
}

/// x₁ = …
fn root_name(v: &str, k: Option<usize>) -> Value {
    match k {
        Some(k) => sub(var(v), t(&k.to_string())),
        None => var(v),
    }
}

#[derive(Clone)]
pub(crate) struct Root {
    pub(crate) exact: Option<E>,
    pub(crate) x: f64,
    pub(crate) mult: u32,
}

fn rq(r: &BigRational) -> E {
    E::Q(r.clone())
}

// ------------------------------------------------------------------ solving

/// Roots of p with textbook steps.
pub(crate) fn roots_with_steps(p: &Poly, v: &str, steps: &mut Vec<Step>) -> Vec<Root> {
    let mut roots: Vec<Root> = Vec::new();
    let mut p = p.clone();
    // x^k common factor
    let k = p.c.iter().take_while(|a| a.is_zero()).count();
    if k > 0 && p.deg() > k as i32 {
        let rest = Poly { c: p.c[k..].to_vec() };
        let xk = if k == 1 { var(v) } else { sup(var(v), t(&k.to_string())) };
        steps.push(step(&format!("Take {v}{} out of the brackets: a product is zero when one of its factors is.", if k > 1 { format!("^{k}") } else { String::new() }),
                        &format!("Выносим {v}{} за скобки: произведение равно нулю, когда хотя бы один множитель равен нулю.", if k > 1 { format!("^{k}") } else { String::new() }))
            .m(row(vec![xk, par(poly_tree(&rest, v)), op("="), t("0")])));
        roots.push(Root { exact: Some(sym::q(0)), x: 0.0, mult: k as u32 });
        p = rest;
    } else if k > 0 {
        roots.push(Root { exact: Some(sym::q(0)), x: 0.0, mult: k as u32 });
        return roots;
    }
    match p.deg() {
        d if d <= 0 => {}
        1 => {
            let (a, b) = (p.coef(1), p.coef(0));
            let x = -b.clone() / a.clone();
            steps.push(step("Linear: move the constant to the right and divide by the coefficient of the unknown.",
                            "Линейное: переносим свободный член вправо и делим на коэффициент при неизвестном.")
                .m(row(vec![rat(&a), var(v), op("="), rat(&-b), op("⇒"), var(v), op("="), rat(&x)])));
            roots.push(Root { exact: Some(rq(&x)), x: x.to_f64().unwrap_or(f64::NAN), mult: 1 });
        }
        2 => roots.extend(quadratic(&p, v, steps)),
        4 if p.coef(1).is_zero() && p.coef(3).is_zero() => {
            // biquadratic: t = x²
            let tq = Poly { c: vec![p.coef(0), p.coef(2), p.coef(4)] };
            steps.push(step(&format!("Biquadratic: substitute t = {v}² (t ≥ 0)."), &format!("Биквадратное: замена t = {v}² (t ≥ 0)."))
                .m(rel_tree(&tq, "t", Rel::Eq)));
            let ts = quadratic(&tq, "t", steps);
            for tr in ts {
                if tr.x < 0.0 {
                    steps.push(step("A negative t is not a square: no roots from it.", "Отрицательное t не может быть квадратом: корней не даёт.")
                        .m(row(vec![var("t"), op("="), tr.exact.as_ref().map(e_tree).unwrap_or(t(&format!("{:.6}", tr.x))), op("<"), t("0")])));
                    continue;
                }
                let e = tr.exact.clone().map(|e| sym::simplify(&E::Pow(Box::new(e), Box::new(sym::qr(1, 2)))));
                steps.push(step(&format!("Back to {v}: {v} = ±√t."), &format!("Возвращаемся к {v}: {v} = ±√t."))
                    .m(row(vec![var(v), op("="), t("±"), e.as_ref().map(e_tree).unwrap_or(t(&format!("{:.6}", tr.x.sqrt())))])));
                let s = tr.x.sqrt();
                if s == 0.0 {
                    roots.push(Root { exact: Some(sym::q(0)), x: 0.0, mult: 2 * tr.mult });
                } else {
                    roots.push(Root { exact: e.clone().map(|e| sym::simplify(&E::Mul(vec![sym::q(-1), e]))), x: -s, mult: tr.mult });
                    roots.push(Root { exact: e, x: s, mult: tr.mult });
                }
            }
        }
        _ => {
            // rational roots + synthetic division until a quadratic (or nothing rational is left)
            let mut cur = p.clone();
            let cands = cur.rational_candidates();
            if cur.deg() > 2 {
                let (ip, _) = cur.integer_form();
                steps.push(step(&format!("Rational-root theorem: a rational root p/q has p dividing {} and q dividing {}. Try them in turn.",
                                         ip.coef(0).abs(), ip.lead().abs()),
                                &format!("Теорема о рациональных корнях: у рационального корня p/q число p делит {}, а q делит {}. Перебираем.",
                                         ip.coef(0).abs(), ip.lead().abs())));
            }
            let mut tried = 0;
            'outer: while cur.deg() > 2 {
                for c in &cands {
                    tried += 1;
                    if tried > 400 {
                        break 'outer;
                    }
                    if cur.eval(c).is_zero() {
                        let (quo, _, horner) = cur.div_linear(c);
                        let mut table = vec![t("coeff.: ")];
                        for (i, a) in cur.c.iter().rev().enumerate() {
                            if i > 0 {
                                table.push(t("  "));
                            }
                            table.push(rat(a));
                        }
                        let mut trow = vec![t("Horner: ")];
                        for (i, a) in horner.iter().enumerate() {
                            if i > 0 {
                                trow.push(t("  "));
                            }
                            trow.push(rat(a));
                        }
                        steps.push(step(&format!("{v} = {} is a root: P({}) = 0. Divide by ({v} − {}) with synthetic division (Horner's scheme); the last number is the remainder 0.",
                                                 sym::plain(&rq(c)), sym::plain(&rq(c)), sym::plain(&rq(c))),
                                        &format!("{v} = {} — корень: P({}) = 0. Делим на ({v} − {}) по схеме Горнера; последнее число — остаток 0.",
                                                 sym::plain(&rq(c)), sym::plain(&rq(c)), sym::plain(&rq(c))))
                            .m(row(table))
                            .m(row(trow))
                            .m(row(vec![poly_tree(&cur, v), op("="), par(row(vec![var(v), op(if c.is_negative() { "+" } else { "−" }), rat(&c.abs())])), op("·"), par(poly_tree(&quo, v))])));
                        // multiplicity: keep dividing while it is still a root
                        let mut mult = 1;
                        cur = quo;
                        while cur.deg() >= 1 && cur.eval(c).is_zero() {
                            cur = cur.div_linear(c).0;
                            mult += 1;
                        }
                        roots.push(Root { exact: Some(rq(c)), x: c.to_f64().unwrap_or(f64::NAN), mult });
                        continue 'outer;
                    }
                }
                break;
            }
            match cur.deg() {
                1 | 2 => roots.extend(roots_with_steps(&cur, v, steps)),
                d if d > 2 => {
                    let r = cur.real_roots();
                    steps.push(step("No rational roots are left: the remaining roots are found numerically.",
                                    "Рациональных корней больше нет: остальные корни находим численно.")
                        .m(rel_tree(&cur, v, Rel::Eq)));
                    for x in r {
                        roots.push(Root { exact: None, x, mult: 1 });
                    }
                }
                _ => {}
            }
        }
    }
    roots
}

fn quadratic(p: &Poly, v: &str, steps: &mut Vec<Step>) -> Vec<Root> {
    let (a, b, c) = (p.coef(2), p.coef(1), p.coef(0));
    if b.is_zero() {
        // a x² + c = 0 → x² = −c/a
        let k = -c.clone() / a.clone();
        let mut s = step(&format!("No {v} term: {v}² = −c/a."), &format!("Нет члена с {v}: {v}² = −c/a."))
            .m(row(vec![sup(var(v), t("2")), op("="), rat(&k)]));
        if k.is_negative() {
            s = s.m(row(vec![t("—")]));
            steps.push(s);
            steps.push(step("A square can't be negative: no real roots.", "Квадрат не может быть отрицательным: действительных корней нет."));
            return vec![];
        }
        let r = sym::simplify(&E::Pow(Box::new(E::Q(k.clone())), Box::new(sym::qr(1, 2))));
        s = s.m(row(vec![var(v), op("="), t("±"), e_tree(&r)]));
        steps.push(s);
        if k.is_zero() {
            return vec![Root { exact: Some(sym::q(0)), x: 0.0, mult: 2 }];
        }
        let x = r.to_f64();
        return vec![Root { exact: Some(sym::simplify(&E::Mul(vec![sym::q(-1), r.clone()]))), x: -x, mult: 1 }, Root { exact: Some(r), x, mult: 1 }];
    }
    if c.is_zero() {
        let x2 = -b.clone() / a.clone();
        steps.push(step(&format!("No constant term: take {v} out: {v}({}) = 0.", "a·x + b"), &format!("Нет свободного члена: выносим {v}: {v}(a·{v} + b) = 0."))
            .m(row(vec![var(v), par(poly_tree(&Poly { c: vec![b.clone(), a.clone()] }, v)), op("="), t("0")]))
            .m(row(vec![var(v), op("="), t("0"), t(",   "), var(v), op("="), rat(&x2)])));
        let mut r = vec![Root { exact: Some(sym::q(0)), x: 0.0, mult: 1 }, Root { exact: Some(rq(&x2)), x: x2.to_f64().unwrap_or(0.0), mult: 1 }];
        r.sort_by(|p, q| p.x.total_cmp(&q.x));
        return r;
    }
    let d = b.clone() * b.clone() - q(4) * a.clone() * c.clone();
    steps.push(step("Quadratic: the discriminant D = b² − 4ac.", "Квадратное: дискриминант D = b² − 4ac.")
        .m(row(vec![var("a"), op("="), rat(&a), t(",  "), var("b"), op("="), rat(&b), t(",  "), var("c"), op("="), rat(&c)]))
        .m(row(vec![var("D"), op("="), sup(par_neg(&b), t("2")), op("−"), t("4"), op("·"), par_neg(&a), op("·"), par_neg(&c), op("="), rat(&d)])));
    let two_a = q(2) * a.clone();
    if d.is_negative() {
        let sq = sym::simplify(&E::Pow(Box::new(E::Q(-d.clone())), Box::new(sym::qr(1, 2))));
        steps.push(step("D < 0: no real roots (the complex ones are x = (−b ± i√|D|)/2a).", "D < 0: действительных корней нет (комплексные: x = (−b ± i√|D|)/2a).")
            .m(row(vec![var(v), op("="), frac(row(vec![rat(&-b.clone()), op("±"), var("i"), e_tree(&sq)]), rat(&two_a))])));
        return vec![];
    }
    if d.is_zero() {
        let x = -b.clone() / two_a.clone();
        steps.push(step("D = 0: one (double) root x = −b/2a.", "D = 0: один (двойной) корень x = −b/2a.")
            .m(row(vec![var(v), op("="), frac(rat(&-b.clone()), rat(&two_a)), op("="), rat(&x)])));
        return vec![Root { exact: Some(rq(&x)), x: x.to_f64().unwrap_or(0.0), mult: 2 }];
    }
    let sqd = sym::simplify(&E::Pow(Box::new(E::Q(d.clone())), Box::new(sym::qr(1, 2))));
    let mk = |sign: i64| {
        sym::simplify(&E::Add(vec![E::Q(-b.clone() / two_a.clone()), E::Mul(vec![E::Q(BigRational::from_integer(BigInt::from(sign)) / two_a.clone()), sqd.clone()])]))
    };
    let (x1, x2) = (mk(-1), mk(1));
    let (mut r1, mut r2) = (Root { x: x1.to_f64(), exact: Some(x1), mult: 1 }, Root { x: x2.to_f64(), exact: Some(x2), mult: 1 });
    if r1.x > r2.x {
        std::mem::swap(&mut r1, &mut r2);
    }
    steps.push(step("D > 0: two roots x₁,₂ = (−b ± √D)/2a.", "D > 0: два корня x₁,₂ = (−b ± √D)/2a.")
        .m(row(vec![sub(var(v), t("1,2")), op("="), frac(row(vec![rat(&-b.clone()), op("±"), sqrt(rat(&d), None)]), rat(&two_a)), op("="),
                    frac(row(vec![rat(&-b.clone()), op("±"), e_tree(&sqd)]), rat(&two_a))]))
        .m(row(vec![root_name(v, Some(1)), op("="), e_tree(r1.exact.as_ref().unwrap()), t(",   "), root_name(v, Some(2)), op("="), e_tree(r2.exact.as_ref().unwrap())])));
    vec![r1, r2]
}

/// a rational, in parentheses when negative or a fraction (for products and powers)
fn par_neg(r: &BigRational) -> Value {
    if r.is_negative() || !r.is_integer() {
        par(rat(r))
    } else {
        rat(r)
    }
}

pub(crate) fn root_tree(r: &Root) -> Value {
    match &r.exact {
        Some(e) => e_tree(e),
        None => row(vec![t("≈ "), t(&crate::num::fmt_float(r.x, 8))]),
    }
}

/// Solve with steps. `orig` is the row as written.
pub fn solve(orig: &Ast, v: &str, rel: Rel, p0: &Poly, already_zero: bool) -> Solution {
    let mut steps = Vec::new();
    let mut p = p0.clone();
    if !already_zero {
        steps.push(step("Move everything to the left side and collect like terms:", "Переносим всё в левую часть и приводим подобные:").m(rel_tree(&p, v, rel)));
    }
    let (ip, k) = p.integer_form();
    if p.c.iter().any(|x| !x.is_integer()) && !k.is_one() {
        steps.push(step(&format!("Multiply by {} to clear the fractions{}:", sym::plain(&E::Q(k.clone())), if rel == Rel::Eq { "" } else { " (a positive number keeps the sign)" }),
                        &format!("Умножаем на {}, чтобы избавиться от дробей{}:", sym::plain(&E::Q(k.clone())), if rel == Rel::Eq { "" } else { " (положительное число не меняет знак)" }))
            .m(rel_tree(&ip, v, rel)));
        p = ip;
    }
    if p.deg() <= 0 {
        let c = p.coef(0);
        let holds = match rel {
            Rel::Eq => c.is_zero(),
            Rel::Ne => !c.is_zero(),
            Rel::Lt => c.is_negative(),
            Rel::Le => !c.is_positive(),
            Rel::Gt => c.is_positive(),
            Rel::Ge => !c.is_negative(),
        };
        let (en, ru) = if holds { ("It holds for every value of the unknown.", "Верно при любом значении неизвестного.") } else { ("It never holds: no solutions.", "Не выполняется ни при каком значении: решений нет.") };
        steps.push(step(en, ru).m(row(vec![rat(&c), op(rel.sym()), t("0")])));
        return Solution {
            en: "Solve".into(),
            ru: "Решение".into(),
            problem: ast(orig),
            steps,
            answer: if holds { row(vec![var(v), op("∈"), t("ℝ")]) } else { t("∅") },
            verified: true,
        };
    }
    let mut roots = roots_with_steps(&p, v, &mut steps);
    roots.sort_by(|a, b| a.x.total_cmp(&b.x));
    // verify every exact root by substitution (numerically for the approximate ones)
    let verified = roots.iter().all(|r| match &r.exact {
        Some(E::Q(x)) => p.eval(x).is_zero(),
        _ => p.eval_f(r.x).abs() < 1e-7 * (1.0 + p.c.iter().map(|a| a.to_f64().unwrap_or(0.0).abs()).fold(0.0, f64::max)),
    });
    if rel == Rel::Eq {
        let answer = if roots.is_empty() {
            text_ru_en("no real roots")
        } else {
            let mut c = Vec::new();
            for (i, r) in roots.iter().enumerate() {
                if i > 0 {
                    c.push(t(";   "));
                }
                c.extend([root_name(v, if roots.len() > 1 { Some(i + 1) } else { None }), op("="), root_tree(r)]);
            }
            row(c)
        };
        return Solution { en: "Solve the equation".into(), ru: "Решение уравнения".into(), problem: ast(orig), steps, answer, verified };
    }
    // interval method
    let lead_pos = p.lead().is_positive();
    let mut bounds: Vec<f64> = roots.iter().map(|r| r.x).collect();
    bounds.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    let mut intervals: Vec<(f64, f64, bool)> = Vec::new(); // (lo, hi, positive)
    let mut edges = vec![f64::NEG_INFINITY];
    edges.extend(bounds.iter().cloned());
    edges.push(f64::INFINITY);
    for w in edges.windows(2) {
        let probe = if w[0].is_infinite() && w[1].is_infinite() {
            0.0
        } else if w[0].is_infinite() {
            w[1] - 1.0
        } else if w[1].is_infinite() {
            w[0] + 1.0
        } else {
            (w[0] + w[1]) / 2.0
        };
        let s = p.eval_f(probe);
        intervals.push((w[0], w[1], if s == 0.0 { lead_pos } else { s > 0.0 }));
    }
    let bound_tree = |x: f64| -> Value {
        if x == f64::NEG_INFINITY {
            return t("−∞");
        }
        if x == f64::INFINITY {
            return t("+∞");
        }
        roots.iter().find(|r| (r.x - x).abs() < 1e-12).map(root_tree).unwrap_or(t(&crate::num::fmt_float(x, 8)))
    };
    let mut chart = Vec::new();
    for (i, (lo, hi, pos)) in intervals.iter().enumerate() {
        if i > 0 {
            chart.push(t("   "));
        }
        chart.extend([t("("), bound_tree(*lo), json!({"k": "t", "s": ", ", "ru": "; "}), bound_tree(*hi), t(")"), t(if *pos { ": +" } else { ": −" })]);
    }
    steps.push(step("Interval method: the roots split the number line into intervals; on each the sign stays the same (check one point in each).",
                    "Метод интервалов: корни разбивают числовую прямую на промежутки, на каждом знак постоянен (проверяем одну точку в каждом).")
        .m(row(chart)));
    let want_pos = matches!(rel, Rel::Gt | Rel::Ge);
    let closed = matches!(rel, Rel::Ge | Rel::Le);
    let mut pieces: Vec<Value> = Vec::new();
    // merge adjacent chosen intervals through a root when the inequality is non-strict
    let mut cur: Option<(f64, f64)> = None;
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (lo, hi, pos) in &intervals {
        let take = *pos == want_pos || (rel == Rel::Ne);
        if take {
            cur = Some(match cur {
                Some((a, b)) if closed && (b - lo).abs() < 1e-12 => (a, *hi),
                Some(seg) => {
                    out.push(seg);
                    (*lo, *hi)
                }
                None => (*lo, *hi),
            });
        } else if let Some(seg) = cur.take() {
            out.push(seg);
        }
    }
    if let Some(seg) = cur {
        out.push(seg);
    }
    // isolated touching roots satisfy a non-strict inequality too
    let mut points: Vec<f64> = Vec::new();
    if closed {
        for r in &roots {
            if !out.iter().any(|(a, b)| r.x >= *a - 1e-12 && r.x <= *b + 1e-12) {
                points.push(r.x);
            }
        }
    }
    for (i, (a, b)) in out.iter().enumerate() {
        if i > 0 {
            pieces.push(op("∪"));
        }
        let lb = if closed && a.is_finite() { "[" } else { "(" };
        let rb = if closed && b.is_finite() { "]" } else { ")" };
        pieces.extend([t(lb), bound_tree(*a), json!({"k": "t", "s": ", ", "ru": "; "}), bound_tree(*b), t(rb)]);
    }
    for x in points {
        if !pieces.is_empty() {
            pieces.push(op("∪"));
        }
        pieces.extend([t("{"), bound_tree(x), t("}")]);
    }
    let answer = if pieces.is_empty() { t("∅") } else { row([vec![var(v), op("∈")], pieces].concat()) };
    steps.push(step(&format!("Take the intervals where the sign is “{}”{}.", if want_pos { "+" } else { "−" }, if closed { ", roots included" } else { "" }),
                    &format!("Берём промежутки со знаком «{}»{}.", if want_pos { "+" } else { "−" }, if closed { ", корни включаем" } else { "" })));
    Solution { en: "Solve the inequality".into(), ru: "Решение неравенства".into(), problem: ast(orig), steps, answer, verified }
}

fn text_ru_en(en: &str) -> Value {
    json!({"k": "t", "s": en, "ru": "нет действительных корней"})
}

/// expand(p) / factor(p) for a polynomial in one unknown: (value as plain text, tree).
pub fn expand_factor(f: &str, arg: &Ast, env: &Env) -> Option<(String, Value)> {
    let mut vars = Vec::new();
    unknown(arg, env, &mut vars);
    let v = match vars.len() {
        0 => "x".to_string(),
        1 => vars[0].clone(),
        _ => return None,
    };
    let p = from_ast(arg, &v, &consts(env))?;
    if f == "expand" {
        return Some((poly_plain(&p, &v), poly_tree(&p, &v)));
    }
    // factor over the rationals: content, rational roots, the rest
    let mut cur = p.clone();
    let mut factors: Vec<(Poly, u32)> = Vec::new();
    let lead = cur.lead();
    let mut kz = 0;
    while cur.deg() > 0 && cur.coef(0).is_zero() {
        cur = Poly { c: cur.c[1..].to_vec() };
        kz += 1;
    }
    if kz > 0 {
        factors.push((Poly::x(), kz));
    }
    for c in cur.rational_candidates() {
        let mut m = 0;
        while cur.deg() >= 1 && cur.eval(&c).is_zero() {
            cur = cur.div_linear(&c).0;
            m += 1;
        }
        if m > 0 {
            // (q·x − p) with integer coefficients
            let lin = Poly { c: vec![-BigRational::from_integer(c.numer().clone()), BigRational::from_integer(c.denom().clone())] };
            factors.push((lin, m));
        }
    }
    // what's left, scaled so the product of the integer factors gives back p
    let mut prod = Poly::constant(q(1));
    for (f, m) in &factors {
        prod = prod.mul(&f.pow(*m));
    }
    let k = if prod.deg() >= 0 && !prod.lead().is_zero() { lead.clone() / prod.lead() / cur.lead() } else { q(1) };
    let rest = cur.clone();
    let mut tree = Vec::new();
    let mut plain = String::new();
    let coef = k.clone() * rest.lead() / rest.lead();
    let _ = coef;
    let restn = rest.scale(&(k.clone()));
    let mut parts: Vec<(Value, String)> = Vec::new();
    if restn.deg() == 0 {
        if !restn.lead().is_one() {
            parts.push((rat(&restn.lead()), sym::plain(&E::Q(restn.lead()))));
        }
    } else {
        parts.push((par(poly_tree(&restn, &v)), format!("({})", poly_plain(&restn, &v))));
    }
    for (f, m) in &factors {
        let base = if f.deg() == 1 && f.coef(0).is_zero() && f.lead().is_one() { (var(&v), v.clone()) } else { (par(poly_tree(f, &v)), format!("({})", poly_plain(f, &v))) };
        if *m > 1 {
            parts.push((sup(base.0, t(&m.to_string())), format!("{}^{m}", base.1)));
        } else {
            parts.push(base);
        }
    }
    for (i, (tr, pl)) in parts.into_iter().enumerate() {
        if i > 0 {
            plain.push('·');
        }
        tree.push(tr);
        plain.push_str(&pl);
    }
    if tree.is_empty() {
        return Some(("0".into(), t("0")));
    }
    Some((plain, row(tree)))
}

pub fn poly_plain(p: &Poly, v: &str) -> String {
    if p.c.is_empty() {
        return "0".into();
    }
    let mut s = String::new();
    for i in (0..p.c.len()).rev() {
        let a = &p.c[i];
        if a.is_zero() {
            continue;
        }
        let neg = a.is_negative();
        let mag = a.abs();
        if s.is_empty() {
            if neg {
                s.push('-');
            }
        } else {
            s.push_str(if neg { " - " } else { " + " });
        }
        let ms = sym::plain(&E::Q(mag.clone()));
        match i {
            0 => s.push_str(&ms),
            _ => {
                if !mag.is_one() {
                    s.push_str(&ms);
                }
                s.push_str(v);
                if i > 1 {
                    s.push_str(&format!("^{i}"));
                }
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::collections::HashSet;

    fn go(src: &str) -> Solution {
        let a = parse(src, &HashSet::new()).unwrap().unwrap();
        let env = Env::default();
        let (v, rel, p, z) = detect(&a, &env).expect(src);
        solve(&a, &v, rel, &p, z)
    }
    fn ans(src: &str) -> String {
        let s = go(src);
        assert!(s.verified, "{src}");
        flat(&s.answer)
    }
    use crate::mtree::flat;

    #[test]
    fn equations() {
        assert_eq!(ans("2x + 3 = 7"), "x=2");
        assert_eq!(ans("x^2 - 5x + 6 = 0"), "x_1=2;   x_2=3");
        assert_eq!(ans("x^2 = 2"), "x_1=−√2;   x_2=√2");
        assert_eq!(ans("x^2 + 1 = 0"), "no real roots");
        assert_eq!(ans("x^3 - 6x^2 + 11x - 6 = 0"), "x_1=1;   x_2=2;   x_3=3");
        assert_eq!(ans("x^4 - 5x^2 + 4 = 0"), "x_1=−2;   x_2=−1;   x_3=1;   x_4=2");
        assert!(ans("x^2 - 4x + 1 = 0").contains('√'));
        assert_eq!(ans("x^2/2 - x = 0"), "x_1=0;   x_2=2");
    }

    #[test]
    fn inequalities() {
        assert_eq!(ans("x^2 - 5x + 6 > 0").replace(' ', ""), "x∈(−∞,2)∪(3,+∞)");
        assert_eq!(ans("x^2 - 5x + 6 <= 0").replace(' ', ""), "x∈[2,3]");
        assert_eq!(ans("2x - 4 < 0").replace(' ', ""), "x∈(−∞,2)");
    }

    #[test]
    fn expand_and_factor() {
        let env = Env::default();
        let p = |s: &str| parse(s, &HashSet::new()).unwrap().unwrap();
        assert_eq!(expand_factor("expand", &p("(x+1)^3"), &env).unwrap().0, "x^3 + 3x^2 + 3x + 1");
        assert_eq!(expand_factor("factor", &p("x^2 - 5x + 6"), &env).unwrap().0, "(x - 2)·(x - 3)");
        assert_eq!(expand_factor("factor", &p("2x^3 - 2x"), &env).unwrap().0, "2·x·(x - 1)·(x + 1)");
    }
}
