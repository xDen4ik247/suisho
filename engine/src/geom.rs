//! Analytic geometry: second-order curves Ax² + Bxy + Cy² + Dx + Ey + F = 0 reduced to canonical form the way
//! the course does it — completing the square when B = 0 (exact), otherwise the invariants δ, Δ, S and a rotation
//! by tg 2θ = B/(A − C) — with the type, semi-axes, eccentricity, foci, directrices and asymptotes.

use crate::eval::{Env, Val};
use crate::mtree::*;
use crate::parse::{Ast, Rel};
use crate::steps::{step, Solution};
use crate::sym::{self, mul, pow, q, qr, E};
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde_json::{json, Value};
use std::collections::HashMap;

type Q = BigRational;
/// coefficients of x^i y^j (i + j ≤ 2)
type Bi = HashMap<(u8, u8), Q>;

fn bi(a: &Ast, env: &Env) -> Option<Bi> {
    let one = |k: (u8, u8), c: Q| -> Bi { HashMap::from([(k, c)]) };
    let addm = |mut a: Bi, b: Bi, s: i64| -> Bi {
        for (k, v) in b {
            *a.entry(k).or_insert_with(Q::zero) += v * Q::from_integer(s.into());
        }
        a
    };
    Some(match a {
        Ast::Num(s) => match crate::num::Num::parse_decimal(s)? {
            crate::num::Num::Q(r) => one((0, 0), r),
            _ => return None,
        },
        Ast::Var(v) if v == "x" => one((1, 0), Q::one()),
        Ast::Var(v) if v == "y" => one((0, 1), Q::one()),
        Ast::Var(v) => match env.vars.get(v) {
            Some(Val::N(crate::num::Num::Q(r))) => one((0, 0), r.clone()),
            _ => return None,
        },
        Ast::Neg(x) => addm(HashMap::new(), bi(x, env)?, -1),
        Ast::Add(x, y) => addm(bi(x, env)?, bi(y, env)?, 1),
        Ast::Sub(x, y) => addm(bi(x, env)?, bi(y, env)?, -1),
        Ast::Mul(x, y, _) => {
            let (p, r) = (bi(x, env)?, bi(y, env)?);
            let mut out: Bi = HashMap::new();
            for ((i1, j1), a) in &p {
                for ((i2, j2), b) in &r {
                    let k = (i1 + i2, j1 + j2);
                    if k.0 + k.1 > 2 {
                        if !(a.is_zero() || b.is_zero()) {
                            return None;
                        }
                        continue;
                    }
                    *out.entry(k).or_insert_with(Q::zero) += a * b;
                }
            }
            out
        }
        Ast::Div(x, y) => {
            let d = bi(y, env)?;
            if d.iter().any(|(k, v)| *k != (0, 0) && !v.is_zero()) {
                return None;
            }
            let c = d.get(&(0, 0)).cloned().unwrap_or_else(Q::zero);
            if c.is_zero() {
                return None;
            }
            bi(x, env)?.into_iter().map(|(k, v)| (k, v / c.clone())).collect()
        }
        Ast::Pow(x, y) => {
            let n: u32 = match &**y {
                Ast::Num(s) => s.parse().ok()?,
                _ => return None,
            };
            let b = bi(x, env)?;
            let mut acc: Bi = HashMap::from([((0, 0), Q::one())]);
            for _ in 0..n {
                acc = bi_mul(&acc, &b)?;
            }
            acc
        }
        _ => return None,
    })
}

fn bi_mul(p: &Bi, r: &Bi) -> Option<Bi> {
    let mut out: Bi = HashMap::new();
    for ((i1, j1), a) in p {
        for ((i2, j2), b) in r {
            if a.is_zero() || b.is_zero() {
                continue;
            }
            let k = (i1 + i2, j1 + j2);
            if k.0 + k.1 > 2 {
                return None;
            }
            *out.entry(k).or_insert_with(Q::zero) += a * b;
        }
    }
    Some(out)
}

/// [A, B, C, D, E, F] of an equation of the second order in x and y (with at least one square/xy term).
pub fn conic_coefs(a: &Ast, env: &Env) -> Option<[Q; 6]> {
    let Ast::Rel(l, ch) = a else { return None };
    if ch.len() != 1 || ch[0].0 != Rel::Eq {
        return None;
    }
    let (pl, pr) = (bi(l, env)?, bi(&ch[0].1, env)?);
    let g = |k: (u8, u8)| pl.get(&k).cloned().unwrap_or_else(Q::zero) - pr.get(&k).cloned().unwrap_or_else(Q::zero);
    let c = [g((2, 0)), g((1, 1)), g((0, 2)), g((1, 0)), g((0, 1)), g((0, 0))];
    if c[0].is_zero() && c[1].is_zero() && c[2].is_zero() {
        return None;
    }
    Some(c)
}

fn rq(r: &Q) -> E {
    E::Q(r.clone())
}
fn root(r: &Q) -> E {
    pow(E::Q(r.clone()), qr(1, 2))
}
fn tq(r: &Q) -> Value {
    rat(r)
}
fn sq(e: Value) -> Value {
    sup(e, t("2"))
}
/// (x − x0) or x, as a tree
fn shifted(v: &str, c: &Q) -> Value {
    if c.is_zero() {
        return var(v);
    }
    par(row(vec![var(v), op(if c.is_negative() { "+" } else { "−" }), tq(&c.abs())]))
}

#[derive(Default)]
pub struct Features {
    pub kind: &'static str,
    pub foci: Vec<(f64, f64)>,
    pub lines: Vec<(f64, f64, f64)>, // a·x + b·y + c = 0 (directrices, asymptotes)
    pub centre: Option<(f64, f64)>,
}

pub fn conic_name(k: &str, ru: bool) -> &'static str {
    match (k, ru) {
        ("ellipse", false) => "ellipse",
        ("ellipse", true) => "эллипс",
        ("circle", false) => "circle",
        ("circle", true) => "окружность",
        ("hyperbola", false) => "hyperbola",
        ("hyperbola", true) => "гипербола",
        ("parabola", false) => "parabola",
        ("parabola", true) => "парабола",
        ("lines", false) => "pair of lines",
        ("lines", true) => "пара прямых",
        ("point", false) => "a point",
        ("point", true) => "точка",
        (_, false) => "no real points",
        (_, true) => "мнимая кривая (нет точек)",
    }
}

/// Classification + numeric features (for the graph overlay and the row label).
pub fn features(c: &[Q; 6]) -> Features {
    let f = |r: &Q| r.to_f64().unwrap_or(0.0);
    let (a, b, cc, d, e, ff) = (f(&c[0]), f(&c[1]), f(&c[2]), f(&c[3]), f(&c[4]), f(&c[5]));
    let delta = a * cc - b * b / 4.0;
    let big = a * (cc * ff - e * e / 4.0) - b / 2.0 * (b / 2.0 * ff - e * d / 4.0) + d / 2.0 * (b / 2.0 * e / 2.0 - cc * d / 2.0);
    let s = a + cc;
    let mut out = Features::default();
    let theta = if b.abs() < 1e-15 { 0.0 } else if (a - cc).abs() < 1e-15 { std::f64::consts::FRAC_PI_4 } else { 0.5 * (b / (a - cc)).atan() };
    let (ct, st) = (theta.cos(), theta.sin());
    let rot = |xx: f64, yy: f64| (xx * ct - yy * st, xx * st + yy * ct); // canonical → original direction
    // eigenvalues in the order matching the rotation by θ
    let l1 = a * ct * ct + b * ct * st + cc * st * st;
    let l2 = a * st * st - b * ct * st + cc * ct * ct;
    let eps = 1e-12 * (1.0 + a.abs() + b.abs() + cc.abs());
    if delta.abs() > eps {
        let x0 = (b * e - 2.0 * cc * d) / (4.0 * a * cc - b * b);
        let y0 = (b * d - 2.0 * a * e) / (4.0 * a * cc - b * b);
        out.centre = Some((x0, y0));
        let fp = big / delta;
        if fp.abs() < 1e-12 {
            out.kind = if delta > 0.0 { "point" } else { "lines" };
            return out;
        }
        let (a2, b2) = (-fp / l1, -fp / l2);
        if delta > 0.0 {
            if a2 < 0.0 && b2 < 0.0 {
                out.kind = "imaginary";
                return out;
            }
            out.kind = if (a2 - b2).abs() < 1e-12 * a2.abs().max(1.0) { "circle" } else { "ellipse" };
            let (maj, min, along_x) = if a2 >= b2 { (a2, b2, true) } else { (b2, a2, false) };
            let cdist = (maj - min).sqrt();
            let dir = if along_x { rot(1.0, 0.0) } else { rot(0.0, 1.0) };
            if out.kind == "ellipse" {
                out.foci = vec![(x0 + cdist * dir.0, y0 + cdist * dir.1), (x0 - cdist * dir.0, y0 - cdist * dir.1)];
                let ecc = cdist / maj.sqrt();
                let dd = maj.sqrt() / ecc;
                for sgn in [1.0, -1.0] {
                    // line through centre + sgn·dd·dir, perpendicular to dir
                    let (px, py) = (x0 + sgn * dd * dir.0, y0 + sgn * dd * dir.1);
                    out.lines.push((dir.0, dir.1, -(dir.0 * px + dir.1 * py)));
                }
            } else {
                out.foci = vec![(x0, y0)];
            }
        } else {
            out.kind = "hyperbola";
            // the real axis is the one with a positive denominator
            let (ra2, ib2, along_x) = if a2 > 0.0 { (a2, -b2, true) } else { (b2, -a2, false) };
            let dir = if along_x { rot(1.0, 0.0) } else { rot(0.0, 1.0) };
            let nrm = if along_x { rot(0.0, 1.0) } else { rot(1.0, 0.0) };
            let cdist = (ra2 + ib2).sqrt();
            out.foci = vec![(x0 + cdist * dir.0, y0 + cdist * dir.1), (x0 - cdist * dir.0, y0 - cdist * dir.1)];
            let k = (ib2 / ra2).sqrt();
            for sgn in [1.0, -1.0] {
                // asymptote direction: dir + sgn·k·nrm; line through the centre
                let (ux, uy) = (dir.0 + sgn * k * nrm.0, dir.1 + sgn * k * nrm.1);
                out.lines.push((uy, -ux, -(uy * x0 - ux * y0)));
            }
            let ecc = cdist / ra2.sqrt();
            let dd = ra2.sqrt() / ecc;
            for sgn in [1.0, -1.0] {
                let (px, py) = (x0 + sgn * dd * dir.0, y0 + sgn * dd * dir.1);
                out.lines.push((dir.0, dir.1, -(dir.0 * px + dir.1 * py)));
            }
        }
    } else if big.abs() > 1e-12 {
        out.kind = "parabola";
        // rotate so the square term is X²: lam·X² + dp·X + ep·Y + F = 0 (or with Y²)
        let (dp, ep) = (d * ct + e * st, -d * st + e * ct);
        let (lam_x, lam_y) = (l1, l2);
        if lam_x.abs() > lam_y.abs() {
            // lam (X + dp/2lam)² = −ep (Y − Y0)
            let xv = -dp / (2.0 * lam_x);
            let yv = -(ff - dp * dp / (4.0 * lam_x)) / ep;
            let p2 = -ep / lam_x; // (X − xv)² = p2 (Y − yv), p2 = 2p
            let (fx, fy) = rot(xv, yv + p2 / 4.0);
            out.foci = vec![(fx, fy)];
            let (dx_, dy_) = rot(xv, yv - p2 / 4.0);
            let n = rot(0.0, 1.0);
            out.lines.push((n.0, n.1, -(n.0 * dx_ + n.1 * dy_)));
            out.centre = Some(rot(xv, yv));
        } else {
            let yv = -ep / (2.0 * lam_y);
            let xv = -(ff - ep * ep / (4.0 * lam_y)) / dp;
            let p2 = -dp / lam_y;
            let (fx, fy) = rot(xv + p2 / 4.0, yv);
            out.foci = vec![(fx, fy)];
            let (dx_, dy_) = rot(xv - p2 / 4.0, yv);
            let n = rot(1.0, 0.0);
            out.lines.push((n.0, n.1, -(n.0 * dx_ + n.1 * dy_)));
            out.centre = Some(rot(xv, yv));
        }
    } else {
        out.kind = "lines";
    }
    let _ = s;
    out
}

/// The step-by-step reduction to canonical form.
pub fn conic_solution(a: &Ast, c: &[Q; 6]) -> Solution {
    let [ca, cb, cc, cd, ce, cf] = c.clone();
    let half = |r: &Q| r / Q::from_integer(2.into());
    let mut st = Vec::new();
    st.push(step("Read the coefficients of Ax² + Bxy + Cy² + Dx + Ey + F = 0:", "Выписываем коэффициенты Ax² + Bxy + Cy² + Dx + Ey + F = 0:")
        .m(row(vec![var("A"), op("="), tq(&ca), t(",  "), var("B"), op("="), tq(&cb), t(",  "), var("C"), op("="), tq(&cc), t(",  "),
                    var("D"), op("="), tq(&cd), t(",  "), var("E"), op("="), tq(&ce), t(",  "), var("F"), op("="), tq(&cf)])));
    // invariants
    let delta = ca.clone() * cc.clone() - half(&cb) * half(&cb);
    let m3 = [[ca.clone(), half(&cb), half(&cd)], [half(&cb), cc.clone(), half(&ce)], [half(&cd), half(&ce), cf.clone()]];
    let big = m3[0][0].clone() * (m3[1][1].clone() * m3[2][2].clone() - m3[1][2].clone() * m3[2][1].clone())
        - m3[0][1].clone() * (m3[1][0].clone() * m3[2][2].clone() - m3[1][2].clone() * m3[2][0].clone())
        + m3[0][2].clone() * (m3[1][0].clone() * m3[2][1].clone() - m3[1][1].clone() * m3[2][0].clone());
    let s = ca.clone() + cc.clone();
    let fe = features(c);
    let ftype = fe.kind;
    let mat3 = json!({"k": "mat", "o": "|", "r": m3.iter().map(|r| r.iter().map(rat).collect::<Vec<_>>()).collect::<Vec<_>>()});
    st.push(step("Invariants (they don't change under rotations and shifts): δ decides the type (> 0 elliptic, < 0 hyperbolic, = 0 parabolic), Δ = 0 means a degenerate curve.",
                 "Инварианты (не меняются при поворотах и сдвигах): δ определяет тип (> 0 эллиптический, < 0 гиперболический, = 0 параболический), Δ = 0 — кривая вырождена.")
        .m(row(vec![t("δ"), op("="), json!({"k": "mat", "o": "|", "r": [[rat(&ca), rat(&half(&cb))], [rat(&half(&cb)), rat(&cc)]]}), op("="), tq(&delta), t(",    "), var("S"), op("="), var("A"), op("+"), var("C"), op("="), tq(&s)]))
        .m(row(vec![t("Δ"), op("="), mat3, op("="), tq(&big)])));
    let name_en = conic_name(ftype, false);
    let name_ru = conic_name(ftype, true);
    let mut answer = t(name_en);
    if cb.is_zero() {
        // complete the square, exactly
        let (x0, y0) = (if ca.is_zero() { Q::zero() } else { -cd.clone() / (Q::from_integer(2.into()) * ca.clone()) },
                        if cc.is_zero() { Q::zero() } else { -ce.clone() / (Q::from_integer(2.into()) * cc.clone()) });
        if !ca.is_zero() && !cc.is_zero() {
            let r = ca.clone() * x0.clone() * x0.clone() + cc.clone() * y0.clone() * y0.clone() - cf.clone();
            st.push(step("No xy term (B = 0): complete the squares in x and in y.", "Нет члена xy (B = 0): выделяем полные квадраты по x и по y.")
                .m(row(vec![coef(&ca), sq(shifted("x", &x0)), op(if cc.is_negative() { "−" } else { "+" }), coef(&cc.abs()), sq(shifted("y", &y0)), op("="), tq(&r)])));
            if r.is_zero() {
                answer = t(if delta.is_positive() { "point" } else { "pair of lines" });
            } else {
                let (a2, b2) = (r.clone() / ca.clone(), r.clone() / cc.clone());
                let canon = |u2: &Q, v2: &Q, minus: bool| row(vec![frac(sq(shifted("x", &x0)), tq(u2)), op(if minus { "−" } else { "+" }), frac(sq(shifted("y", &y0)), tq(v2)), op("="), t("1")]);
                if a2.is_positive() && b2.is_positive() {
                    st.push(step(&format!("Divide by {}: the canonical equation of an {}.", sym::plain(&rq(&r)), if a2 == b2 { "circle" } else { "ellipse" }),
                                 &format!("Делим на {}: каноническое уравнение {}.", sym::plain(&rq(&r)), if a2 == b2 { "окружности" } else { "эллипса" }))
                        .m(canon(&a2, &b2, false)));
                    let (maj, min) = if a2 >= b2 { (a2.clone(), b2.clone()) } else { (b2.clone(), a2.clone()) };
                    let ae = root(&maj);
                    let be = root(&min);
                    let c2 = maj.clone() - min.clone();
                    let ce_ = root(&c2);
                    let ecc = sym::simplify(&E::Mul(vec![ce_.clone(), E::Pow(Box::new(ae.clone()), Box::new(q(-1)))]));
                    let centre = par(row(vec![tq(&x0), t("; "), tq(&y0)]));
                    if a2 == b2 {
                        st.push(step("Centre and radius:", "Центр и радиус:").m(row(vec![t("centre "), centre.clone(), t(",   "), var("R"), op("="), sym_t(&ae)])));
                        answer = row(vec![t("circle, centre "), centre, t(", R = "), sym_t(&ae)]);
                    } else {
                        let along = if a2 >= b2 { "x" } else { "y" };
                        st.push(step(&format!("Semi-axes a ≥ b (the major axis is along {along}), c² = a² − b², eccentricity e = c/a < 1."),
                                     &format!("Полуоси a ≥ b (большая ось вдоль {along}), c² = a² − b², эксцентриситет e = c/a < 1."))
                            .m(row(vec![var("a"), op("="), sym_t(&ae), t(",   "), var("b"), op("="), sym_t(&be), t(",   "), var("c"), op("="), sym_t(&ce_), t(",   "), var("e"), op("="), sym_t(&ecc)]))
                            .m(row(vec![t("centre "), centre.clone(), t(",   foci: centre ± c along the major axis")])));
                        answer = row(vec![t("ellipse, centre "), centre, t(", a = "), sym_t(&ae), t(", b = "), sym_t(&be), t(", e = "), sym_t(&ecc)]);
                    }
                } else if a2.is_negative() && b2.is_negative() {
                    st.push(step("A sum of squares can't be negative: no real points (an imaginary ellipse).", "Сумма квадратов не может быть отрицательной: точек нет (мнимый эллипс).")
                        .m(canon(&-a2.clone(), &-b2.clone(), false)));
                    answer = t("no real points");
                } else {
                    let (pa, pb, xfirst) = if a2.is_positive() { (a2.clone(), -b2.clone(), true) } else { (b2.clone(), -a2.clone(), false) };
                    let eq_tree = if xfirst { canon(&pa, &pb, true) } else { row(vec![frac(sq(shifted("y", &y0)), tq(&pa)), op("−"), frac(sq(shifted("x", &x0)), tq(&pb)), op("="), t("1")]) };
                    let ae = root(&pa);
                    let be = root(&pb);
                    let ce_ = root(&(pa.clone() + pb.clone()));
                    let ecc = sym::simplify(&E::Mul(vec![ce_.clone(), E::Pow(Box::new(ae.clone()), Box::new(q(-1)))]));
                    let kk = sym::simplify(&E::Mul(vec![be.clone(), E::Pow(Box::new(ae.clone()), Box::new(q(-1)))]));
                    st.push(step(&format!("Divide by {}: the canonical equation of a hyperbola.", sym::plain(&rq(&r))), &format!("Делим на {}: каноническое уравнение гиперболы.", sym::plain(&rq(&r)))).m(eq_tree));
                    st.push(step("Semi-axes, c² = a² + b², e = c/a > 1, asymptotes through the centre with slope ±b/a (relative to the real axis).",
                                 "Полуоси, c² = a² + b², e = c/a > 1, асимптоты через центр с угловым коэффициентом ±b/a (относительно действительной оси).")
                        .m(row(vec![var("a"), op("="), sym_t(&ae), t(",   "), var("b"), op("="), sym_t(&be), t(",   "), var("c"), op("="), sym_t(&ce_), t(",   "), var("e"), op("="), sym_t(&ecc), t(",   b/a = "), sym_t(&kk)])));
                    answer = row(vec![t("hyperbola, centre "), par(row(vec![tq(&x0), t("; "), tq(&y0)])), t(", a = "), sym_t(&ae), t(", b = "), sym_t(&be), t(", e = "), sym_t(&ecc)]);
                }
            }
        } else {
            // parabola: one square
            let (sqv, sqc, sq0, lin_v, lin_c) = if !ca.is_zero() { ("x", ca.clone(), x0.clone(), "y", ce.clone()) } else { ("y", cc.clone(), y0.clone(), "x", cd.clone()) };
            if lin_c.is_zero() {
                answer = t("pair of parallel lines (or none)");
                st.push(step("Only one variable is left: a pair of parallel lines, one line or nothing.", "Осталась одна переменная: пара параллельных прямых, одна прямая или ничего."));
            } else {
                // sqc (v − v0)² + lin_c·w + (F − sqc·v0²) = 0 → (v − v0)² = −lin_c/sqc · (w − w0)
                let rest = cf.clone() - sqc.clone() * sq0.clone() * sq0.clone();
                let w0 = -rest / lin_c.clone();
                let two_p = -lin_c.clone() / sqc.clone();
                st.push(step(&format!("Only {sqv}² is present: complete the square in {sqv} and move the rest to the right side."), &format!("Есть только {sqv}²: выделяем полный квадрат по {sqv}, остальное переносим вправо."))
                    .m(row(vec![sq(shifted(sqv, &sq0)), op("="), tq(&two_p), shifted(lin_v, &w0)])));
                let p = two_p.clone() / Q::from_integer(2.into());
                let vertex = if sqv == "x" { (sq0.clone(), w0.clone()) } else { (w0.clone(), sq0.clone()) };
                st.push(step("The canonical form (v − v₀)² = 2p(w − w₀): the vertex, the parameter p, the focus at distance p/2 from the vertex, the directrix on the other side.",
                             "Каноническое уравнение (v − v₀)² = 2p(w − w₀): вершина, параметр p, фокус на расстоянии p/2 от вершины, директриса по другую сторону.")
                    .m(row(vec![t("vertex "), par(row(vec![tq(&vertex.0), t("; "), tq(&vertex.1)])), t(",   "), var("p"), op("="), tq(&p)])));
                answer = row(vec![t("parabola, vertex "), par(row(vec![tq(&vertex.0), t("; "), tq(&vertex.1)])), t(", p = "), tq(&p)]);
            }
        }
    } else {
        // rotation, then the invariants give the canonical form
        let amc = ca.clone() - cc.clone();
        st.push(step("There is an xy term: rotate the axes by θ with tg 2θ = B/(A − C) (θ = 45° when A = C) to remove it.",
                     "Есть член xy: поворачиваем оси на угол θ, tg 2θ = B/(A − C) (θ = 45° при A = C), чтобы его убрать.")
            .m(if amc.is_zero() { row(vec![t("A = C ⇒ θ = 45°")]) } else {
                row(vec![t("tg 2θ"), op("="), frac(tq(&cb), tq(&amc)), t(",   θ ≈ "), t(&format!("{:.4}°", 0.5 * (cb.to_f64().unwrap() / amc.to_f64().unwrap()).atan().to_degrees()))])
            }));
        // λ1, λ2 = (S ± √((A − C)² + B²))/2
        let disc = amc.clone() * amc.clone() + cb.clone() * cb.clone();
        let sd = root(&disc);
        let l1 = sym::simplify(&E::Add(vec![rq(&half(&s)), mul(vec![qr(1, 2), sd.clone()])]));
        let l2 = sym::simplify(&E::Add(vec![rq(&half(&s)), mul(vec![qr(-1, 2), sd.clone()])]));
        st.push(step("The new coefficients of X² and Y² are the roots of λ² − Sλ + δ = 0:", "Новые коэффициенты при X² и Y² — корни λ² − Sλ + δ = 0:")
            .m(row(vec![sub(t("λ"), t("1,2")), op("="), frac(row(vec![tq(&s), op("±"), sqrt(tq(&disc), None)]), t("2")), t(":   "), sym_t(&l1), t(",  "), sym_t(&l2)])));
        if !delta.is_zero() {
            let fp = big.clone() / delta.clone();
            st.push(step("Move to the centre (it solves Aₓ = 0, A_y = 0); the free term becomes Δ/δ:", "Переносим начало в центр (решение системы F′ₓ = 0, F′_y = 0); свободный член становится Δ/δ:")
                .m(row(vec![sym_t(&l1), sq(var("X")), op("+"), sym_t(&l2), sq(var("Y")), op("+"), tq(&fp), op("="), t("0")])));
        }
        answer = t(&format!("{}{}", name_en, fe.centre.map(|(x, y)| format!(", centre ({}; {})", crate::num::fmt_float(x, 5), crate::num::fmt_float(y, 5))).unwrap_or_default()));
    }
    let mut out = vec![];
    out.push(answer);
    if !fe.foci.is_empty() && ftype != "circle" {
        let fs: Vec<String> = fe.foci.iter().map(|(x, y)| format!("({}; {})", crate::num::fmt_float(*x, 5), crate::num::fmt_float(*y, 5))).collect();
        st.push(step(&format!("Foci (drawn on the graph): {}", fs.join(", ")), &format!("Фокусы (отмечены на графике): {}", fs.join(", "))));
    }
    let _ = name_ru;
    Solution { en: "Second-order curve".into(), ru: "Кривая второго порядка".into(), problem: ast(a), steps: st, answer: row(out), verified: true }
}

/// a coefficient in front of a bracket: nothing for 1, "−" for −1
fn coef(r: &Q) -> Value {
    if r.is_one() {
        row(vec![])
    } else if *r == -Q::one() {
        t("−")
    } else {
        tq(r)
    }
}

fn sym_t(e: &E) -> Value {
    crate::mtree::sym(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::collections::HashSet;

    fn c(src: &str) -> [Q; 6] {
        conic_coefs(&parse(src, &HashSet::new()).unwrap().unwrap(), &Env::default()).unwrap()
    }

    fn con(src: &str) -> Solution {
        let a = parse(src, &HashSet::new()).unwrap().unwrap();
        construct(&a, &mut Env::default()).unwrap().unwrap()
    }

    #[test]
    fn constructions() {
        assert_eq!(crate::mtree::flat(&con("line((1,2),(3,6))").answer), "2x−y=0");
        assert_eq!(crate::mtree::flat(&con("distance((0,0),(3,4))").answer), "5");
        assert_eq!(crate::mtree::flat(&con("distance((1,1),(2,2))").answer), "√2");
        assert_eq!(crate::mtree::flat(&con("distance((0,0), 3x + 4y - 10 = 0)").answer), "2");
        assert_eq!(crate::mtree::flat(&con("cross((1,0,0),(0,1,0))").answer), "(0, 0, 1)");
        assert_eq!(crate::mtree::flat(&con("plane((1,0,0),(0,1,0),(0,0,1))").answer), "x+y+z−1=0");
        let tri = crate::mtree::flat(&con("triangle((0,0),(4,0),(0,3))").answer);
        assert!(tri.starts_with("S = 6,  P = 12"), "{tri}");
    }

    #[test]
    fn classify() {
        assert_eq!(features(&c("x^2 + 4y^2 - 2x + 16y + 1 = 0")).kind, "ellipse");
        assert_eq!(features(&c("x^2 + y^2 = 9")).kind, "circle");
        assert_eq!(features(&c("x^2 - y^2 = 1")).kind, "hyperbola");
        assert_eq!(features(&c("y^2 = 4x")).kind, "parabola");
        assert_eq!(features(&c("x y = 1")).kind, "hyperbola");
        assert_eq!(features(&c("5x^2 + 4x y + 8y^2 - 36 = 0")).kind, "ellipse");
        assert_eq!(features(&c("x^2 - y^2 = 0")).kind, "lines");
        // y² = 4x: focus (1, 0), directrix x = −1
        let f = features(&c("y^2 = 4x"));
        assert!((f.foci[0].0 - 1.0).abs() < 1e-9 && f.foci[0].1.abs() < 1e-9, "{:?}", f.foci);
        // x²/25 + y²/9 = 1: foci (±4, 0)
        let f = features(&c("x^2/25 + y^2/9 = 1"));
        assert!(f.foci.iter().any(|p| (p.0 - 4.0).abs() < 1e-9));
        let s = conic_solution(&parse("x^2 + 4y^2 - 2x + 16y + 1 = 0", &HashSet::new()).unwrap().unwrap(), &c("x^2 + 4y^2 - 2x + 16y + 1 = 0"));
        assert!(crate::mtree::flat(&s.answer).starts_with("ellipse, centre (1; −2), a = 4, b = 2"), "{}", crate::mtree::flat(&s.answer));
    }
}

// ------------------------------------------------------------------ constructions with points and vectors

fn point(a: &Ast, env: &mut Env) -> Result<Vec<Q>, String> {
    match env.eval(a, &HashMap::new()) {
        Ok(Val::Tuple(v)) | Ok(Val::List(v)) => v
            .iter()
            .map(|x| match x {
                Val::N(crate::num::Num::Q(r)) => Ok(r.clone()),
                Val::N(n) => Q::from_float(n.to_f64()).ok_or_else(|| "not a number".to_string()),
                _ => Err("a point/vector has number coordinates".to_string()),
            })
            .collect(),
        Ok(_) => Err("expected a point like (1, 2)".into()),
        Err(crate::eval::EvalErr::Free(n)) => Err(format!("{n} is not defined")),
        Err(crate::eval::EvalErr::Msg(m)) => Err(m),
    }
}

fn pt(p: &[Q]) -> Value {
    let mut c = Vec::new();
    for (i, x) in p.iter().enumerate() {
        if i > 0 {
            c.push(json!({"k": "t", "s": ", ", "ru": "; "}));
        }
        c.push(rat(x));
    }
    par(row(c))
}

fn sqrt_e(r: &Q) -> E {
    sym::simplify(&E::Pow(Box::new(E::Q(r.clone())), Box::new(qr(1, 2))))
}

fn dist2(p: &[Q], q_: &[Q]) -> Q {
    p.iter().zip(q_).map(|(a, b)| (b - a) * (b - a)).sum()
}

/// integer coefficients a·x + b·y + c (scaled by the lcm of denominators, divided by the gcd)
fn int_line(v: &[Q]) -> Vec<Q> {
    use num_integer::Integer;
    let l = v.iter().fold(num_bigint::BigInt::one(), |acc, x| acc.lcm(x.denom()));
    let iv: Vec<num_bigint::BigInt> = v.iter().map(|x| (x * Q::from_integer(l.clone())).to_integer()).collect();
    let g = iv.iter().fold(num_bigint::BigInt::zero(), |acc, x| acc.gcd(x));
    let g = if g.is_zero() { num_bigint::BigInt::one() } else { g };
    // a positive leading coefficient
    let sgn = if iv.iter().find(|x| !x.is_zero()).is_some_and(|x| x.is_negative()) { -1 } else { 1 };
    iv.iter().map(|x| Q::from_integer(x / &g * sgn)).collect()
}

/// a·x + b·y (+ c·z) + d = 0 as a tree
fn general(coefs: &[Q], vars: &[&str]) -> Value {
    let mut c = Vec::new();
    for (i, k) in coefs.iter().enumerate() {
        if k.is_zero() {
            continue;
        }
        if c.is_empty() {
            if k.is_negative() {
                c.push(t("−"));
            }
        } else {
            c.push(op(if k.is_negative() { "−" } else { "+" }));
        }
        let m = k.abs();
        if i < vars.len() {
            if !m.is_one() {
                c.push(rat(&m));
            }
            c.push(var(vars[i]));
        } else {
            c.push(rat(&m));
        }
    }
    if c.is_empty() {
        c.push(t("0"));
    }
    c.extend([op("="), t("0")]);
    row(c)
}

/// Linear equation in x, y (and z) → coefficients [a, b, (c,) d] of a·x + b·y + (c·z) + d = 0.
fn linear_eq(a: &Ast, env: &Env, vars: &[&str]) -> Option<Vec<Q>> {
    let Ast::Rel(l, ch) = a else { return None };
    if ch.len() != 1 || ch[0].0 != Rel::Eq {
        return None;
    }
    // reuse the bivariate reader for x, y; z via substitution trick is not needed for 2D
    if vars.len() == 2 {
        let (pl, pr) = (bi(l, env)?, bi(&ch[0].1, env)?);
        let g = |k: (u8, u8)| pl.get(&k).cloned().unwrap_or_else(Q::zero) - pr.get(&k).cloned().unwrap_or_else(Q::zero);
        if !(g((2, 0)).is_zero() && g((1, 1)).is_zero() && g((0, 2)).is_zero()) {
            return None;
        }
        return Some(vec![g((1, 0)), g((0, 1)), g((0, 0))]);
    }
    None
}

pub fn construct(a: &Ast, env: &mut Env) -> Option<Result<Solution, String>> {
    let Ast::Call(f, args) = a else { return None };
    let f = f.as_str();
    if !matches!(f, "line" | "distance" | "midpoint" | "triangle" | "plane" | "dot" | "cross" | "angle") {
        return None;
    }
    let problem = ast(a);
    Some((|| -> Result<Solution, String> {
        let sol = |en: &str, ru: &str, steps, answer| Solution { en: en.into(), ru: ru.into(), problem: problem.clone(), steps, answer, verified: true };
        match (f, args.len()) {
            ("line", 2) => {
                let (p, q_) = (point(&args[0], env)?, point(&args[1], env)?);
                if p.len() != 2 || q_.len() != 2 || p == q_ {
                    return Err("line(P, Q) needs two different points in the plane".into());
                }
                let (dx, dy) = (q_[0].clone() - p[0].clone(), q_[1].clone() - p[1].clone());
                let co = int_line(&[dy.clone(), -dx.clone(), dx.clone() * p[1].clone() - dy.clone() * p[0].clone()]);
                let mut st = vec![step("Line through two points: (x − x₁)/(x₂ − x₁) = (y − y₁)/(y₂ − y₁).", "Прямая через две точки: (x − x₁)/(x₂ − x₁) = (y − y₁)/(y₂ − y₁).")
                    .m(row(vec![frac(shifted("x", &p[0]), rat(&dx)), op("="), frac(shifted("y", &p[1]), rat(&dy))]))];
                st.push(step("Cross-multiply and collect: the general equation ax + by + c = 0.", "Перемножаем крест-накрест и приводим: общее уравнение ax + by + c = 0.").m(general(&co, &["x", "y"])));
                if !co[1].is_zero() {
                    let k = -co[0].clone() / co[1].clone();
                    let m = -co[2].clone() / co[1].clone();
                    st.push(step("Slope form y = kx + m (k is the slope):", "Уравнение с угловым коэффициентом y = kx + m:")
                        .m(row(vec![var("y"), op("="), crate::algebra::poly_tree(&crate::poly::Poly { c: vec![m, k] }, "x")])));
                }
                Ok(sol("Line through two points", "Прямая через две точки", st, general(&co, &["x", "y"])))
            }
            ("distance", 2) => {
                let p = point(&args[0], env)?;
                if let Ok(q_) = point(&args[1], env) {
                    if p.len() != q_.len() {
                        return Err("the points have different dimensions".into());
                    }
                    let d2 = dist2(&p, &q_);
                    let d = sqrt_e(&d2);
                    let mut sum = Vec::new();
                    for (i, (a_, b_)) in p.iter().zip(&q_).enumerate() {
                        if i > 0 {
                            sum.push(op("+"));
                        }
                        sum.push(sq(par(row(vec![rat(b_), op("−"), crate::mtree::num_p(&crate::num::Num::Q(a_.clone()))]))));
                    }
                    let st = vec![step("Distance between points: d = √(Σ(qᵢ − pᵢ)²).", "Расстояние между точками: d = √(Σ(qᵢ − pᵢ)²).")
                        .m(row(vec![var("d"), op("="), sqrt(row(sum), None), op("="), sqrt(rat(&d2), None), op("="), sym_t(&d)]))];
                    return Ok(sol("Distance", "Расстояние", st, sym_t(&d)));
                }
                // point to a line a·x + b·y + c = 0
                let co = linear_eq(&args[1], env, &["x", "y"]).ok_or("distance(P, Q) or distance(P, ax + by + c = 0)")?;
                if p.len() != 2 {
                    return Err("a point in the plane".into());
                }
                let num = co[0].clone() * p[0].clone() + co[1].clone() * p[1].clone() + co[2].clone();
                let den2 = co[0].clone() * co[0].clone() + co[1].clone() * co[1].clone();
                let d = sym::simplify(&E::Mul(vec![E::Q(num.abs()), E::Pow(Box::new(E::Q(den2.clone())), Box::new(qr(-1, 2)))]));
                let st = vec![step("Distance from a point to a line: d = |a·x₀ + b·y₀ + c| / √(a² + b²).", "Расстояние от точки до прямой: d = |a·x₀ + b·y₀ + c| / √(a² + b²).")
                    .m(row(vec![var("d"), op("="), frac(bars(rat(&num)), sqrt(rat(&den2), None)), op("="), sym_t(&d)]))];
                Ok(sol("Distance from a point to a line", "Расстояние от точки до прямой", st, sym_t(&d)))
            }
            ("midpoint", 2) => {
                let (p, q_) = (point(&args[0], env)?, point(&args[1], env)?);
                let m: Vec<Q> = p.iter().zip(&q_).map(|(a_, b_)| (a_ + b_) / Q::from_integer(2.into())).collect();
                let st = vec![step("Midpoint: the average of the coordinates.", "Середина отрезка: полусумма координат.").m(row(vec![var("M"), op("="), pt(&m)]))];
                Ok(sol("Midpoint", "Середина отрезка", st, pt(&m)))
            }
            ("dot" | "angle", 2) => {
                let (u, w) = (point(&args[0], env)?, point(&args[1], env)?);
                if u.len() != w.len() {
                    return Err("vectors of the same dimension".into());
                }
                let d: Q = u.iter().zip(&w).map(|(a_, b_)| a_ * b_).sum();
                let mut terms = Vec::new();
                for (i, (a_, b_)) in u.iter().zip(&w).enumerate() {
                    if i > 0 {
                        terms.push(op("+"));
                    }
                    terms.extend([crate::mtree::num_p(&crate::num::Num::Q(a_.clone())), op("·"), crate::mtree::num_p(&crate::num::Num::Q(b_.clone()))]);
                }
                let mut st = vec![step("Dot product: the sum of the products of the coordinates.", "Скалярное произведение: сумма произведений координат.")
                    .m(row([vec![t("a·b"), op("=")], terms, vec![op("="), rat(&d)]].concat()))];
                if f == "dot" {
                    return Ok(sol("Dot product", "Скалярное произведение", st, rat(&d)));
                }
                let (n1, n2) = (u.iter().map(|x| x * x).sum::<Q>(), w.iter().map(|x| x * x).sum::<Q>());
                let cosv = sym::simplify(&E::Mul(vec![E::Q(d.clone()), E::Pow(Box::new(E::Q(n1.clone() * n2.clone())), Box::new(qr(-1, 2)))]));
                let deg = cosv.to_f64().clamp(-1.0, 1.0).acos().to_degrees();
                st.push(step("cos φ = a·b / (|a|·|b|).", "cos φ = a·b / (|a|·|b|).")
                    .m(row(vec![t("cos φ"), op("="), frac(rat(&d), row(vec![sqrt(rat(&n1), None), op("·"), sqrt(rat(&n2), None)])), op("="), sym_t(&cosv)]))
                    .m(row(vec![t("φ ≈ "), t(&format!("{:.4}°", deg))])));
                Ok(sol("Angle between vectors", "Угол между векторами", st, row(vec![t("cos φ = "), sym_t(&cosv), t(&format!(",  φ ≈ {:.4}°", deg))])))
            }
            ("cross", 2) => {
                let (u, w) = (point(&args[0], env)?, point(&args[1], env)?);
                if u.len() != 3 || w.len() != 3 {
                    return Err("the cross product is for 3D vectors".into());
                }
                let c = vec![u[1].clone() * w[2].clone() - u[2].clone() * w[1].clone(), u[2].clone() * w[0].clone() - u[0].clone() * w[2].clone(), u[0].clone() * w[1].clone() - u[1].clone() * w[0].clone()];
                let det = json!({"k": "mat", "o": "|", "r": [[t("i"), t("j"), t("k")], u.iter().map(rat).collect::<Vec<_>>(), w.iter().map(rat).collect::<Vec<_>>()]});
                let st = vec![step("Cross product: expand the determinant with i, j, k along the first row.", "Векторное произведение: раскрываем определитель с i, j, k по первой строке.")
                    .m(row(vec![t("a × b"), op("="), det, op("="), pt(&c)]))];
                Ok(sol("Cross product", "Векторное произведение", st, pt(&c)))
            }
            ("plane", 3) => {
                let (p, q_, r) = (point(&args[0], env)?, point(&args[1], env)?, point(&args[2], env)?);
                if p.len() != 3 || q_.len() != 3 || r.len() != 3 {
                    return Err("plane(A, B, C) needs three points in space".into());
                }
                let u: Vec<Q> = q_.iter().zip(&p).map(|(a_, b_)| a_ - b_).collect();
                let w: Vec<Q> = r.iter().zip(&p).map(|(a_, b_)| a_ - b_).collect();
                let n = vec![u[1].clone() * w[2].clone() - u[2].clone() * w[1].clone(), u[2].clone() * w[0].clone() - u[0].clone() * w[2].clone(), u[0].clone() * w[1].clone() - u[1].clone() * w[0].clone()];
                if n.iter().all(|x| x.is_zero()) {
                    return Err("the three points are on one line".into());
                }
                let d = -(n[0].clone() * p[0].clone() + n[1].clone() * p[1].clone() + n[2].clone() * p[2].clone());
                let co = int_line(&[n[0].clone(), n[1].clone(), n[2].clone(), d]);
                let st = vec![
                    step("Two vectors in the plane: AB and AC.", "Два вектора в плоскости: AB и AC.").m(row(vec![t("AB = "), pt(&u), t(",   AC = "), pt(&w)])),
                    step("The normal vector is their cross product n = AB × AC.", "Нормальный вектор — их векторное произведение n = AB × AC.").m(row(vec![var("n"), op("="), pt(&n)])),
                    step("The plane: n·(X − A) = 0, i.e. a(x − x₀) + b(y − y₀) + c(z − z₀) = 0.", "Плоскость: n·(X − A) = 0, то есть a(x − x₀) + b(y − y₀) + c(z − z₀) = 0.").m(general(&co, &["x", "y", "z"])),
                ];
                Ok(sol("Plane through three points", "Плоскость через три точки", st, general(&co, &["x", "y", "z"])))
            }
            ("triangle", 3) => {
                let (pa, pb, pc) = (point(&args[0], env)?, point(&args[1], env)?, point(&args[2], env)?);
                if pa.len() != 2 || pb.len() != 2 || pc.len() != 2 {
                    return Err("triangle(A, B, C) needs three points in the plane".into());
                }
                let two = Q::from_integer(2.into());
                let cross = (pb[0].clone() - pa[0].clone()) * (pc[1].clone() - pa[1].clone()) - (pb[1].clone() - pa[1].clone()) * (pc[0].clone() - pa[0].clone());
                if cross.is_zero() {
                    return Err("the points are on one line (a degenerate triangle)".into());
                }
                let area = cross.abs() / two.clone();
                let (a2, b2, c2) = (dist2(&pb, &pc), dist2(&pa, &pc), dist2(&pa, &pb));
                let (a_, b_, c_) = (sqrt_e(&a2), sqrt_e(&b2), sqrt_e(&c2));
                let per = sym::simplify(&E::Add(vec![a_.clone(), b_.clone(), c_.clone()]));
                let g: Vec<Q> = (0..2).map(|i| (pa[i].clone() + pb[i].clone() + pc[i].clone()) / Q::from_integer(3.into())).collect();
                // circumcentre: equidistant from A, B, C (two linear equations)
                let (ax, ay, bx, by, cx, cy) = (pa[0].clone(), pa[1].clone(), pb[0].clone(), pb[1].clone(), pc[0].clone(), pc[1].clone());
                let dd = two.clone() * (ax.clone() * (by.clone() - cy.clone()) + bx.clone() * (cy.clone() - ay.clone()) + cx.clone() * (ay.clone() - by.clone()));
                let sa = ax.clone() * ax.clone() + ay.clone() * ay.clone();
                let sb = bx.clone() * bx.clone() + by.clone() * by.clone();
                let sc = cx.clone() * cx.clone() + cy.clone() * cy.clone();
                let ox = (sa.clone() * (by.clone() - cy.clone()) + sb.clone() * (cy.clone() - ay.clone()) + sc.clone() * (ay.clone() - by.clone())) / dd.clone();
                let oy = (sa * (cx.clone() - bx.clone()) + sb * (ax.clone() - cx.clone()) + sc * (bx.clone() - ax.clone())) / dd;
                let o = vec![ox, oy];
                let h: Vec<Q> = (0..2).map(|i| pa[i].clone() + pb[i].clone() + pc[i].clone() - two.clone() * o[i].clone()).collect();
                let rr = sqrt_e(&dist2(&o, &pa));
                let ang = |opp2: &Q, s1: &Q, s2: &Q| -> f64 {
                    let (o_, x1, x2) = (opp2.to_f64().unwrap(), s1.to_f64().unwrap(), s2.to_f64().unwrap());
                    ((x1 + x2 - o_) / (2.0 * x1.sqrt() * x2.sqrt())).clamp(-1.0, 1.0).acos().to_degrees()
                };
                let (aa, ab, ac) = (ang(&a2, &b2, &c2), ang(&b2, &a2, &c2), ang(&c2, &a2, &b2));
                let (af, bf, cf) = (a_.to_f64(), b_.to_f64(), c_.to_f64());
                let ps = af + bf + cf;
                let inc = ((af * ax.to_f64().unwrap() + bf * bx.to_f64().unwrap() + cf * cx.to_f64().unwrap()) / ps, (af * ay.to_f64().unwrap() + bf * by.to_f64().unwrap() + cf * cy.to_f64().unwrap()) / ps);
                let st = vec![
                    step("Sides (distances between the vertices): a = BC, b = AC, c = AB.", "Стороны (расстояния между вершинами): a = BC, b = AC, c = AB.")
                        .m(row(vec![var("a"), op("="), sym_t(&a_), t(",   "), var("b"), op("="), sym_t(&b_), t(",   "), var("c"), op("="), sym_t(&c_), t(",   P = "), sym_t(&per)])),
                    step("Area: S = ½·|(x_B − x_A)(y_C − y_A) − (y_B − y_A)(x_C − x_A)|.", "Площадь: S = ½·|(x_B − x_A)(y_C − y_A) − (y_B − y_A)(x_C − x_A)|.")
                        .m(row(vec![var("S"), op("="), frac(t("1"), t("2")), op("·"), bars(rat(&cross)), op("="), rat(&area)])),
                    step("Angles by the law of cosines: cos A = (b² + c² − a²)/(2bc).", "Углы по теореме косинусов: cos A = (b² + c² − a²)/(2bc).")
                        .m(t(&format!("A ≈ {aa:.4}°,   B ≈ {ab:.4}°,   C ≈ {ac:.4}°   (sum 180°)"))),
                    step("Centroid (medians meet): the average of the vertices.", "Центр масс (точка пересечения медиан): среднее вершин.").m(row(vec![var("G"), op("="), pt(&g)])),
                    step("Circumcentre (perpendicular bisectors meet): equidistant from A, B, C; R = |OA|.", "Центр описанной окружности (пересечение серединных перпендикуляров): равноудалён от вершин; R = |OA|.")
                        .m(row(vec![var("O"), op("="), pt(&o), t(",   R = "), sym_t(&rr)])),
                    step("Orthocentre (altitudes meet): H = A + B + C − 2O.", "Ортоцентр (пересечение высот): H = A + B + C − 2O.").m(row(vec![var("H"), op("="), pt(&h)])),
                    step("Incentre (bisectors meet): (a·A + b·B + c·C)/(a + b + c); r = S/p.", "Центр вписанной окружности (пересечение биссектрис): (a·A + b·B + c·C)/(a + b + c); r = S/p.")
                        .m(t(&format!("I ≈ ({:.5}; {:.5}),   r ≈ {:.5}", inc.0, inc.1, area.to_f64().unwrap() / (ps / 2.0)))),
                ];
                Ok(sol("Triangle", "Треугольник", st, row(vec![t("S = "), rat(&area), t(",  P = "), sym_t(&per), t(",  G = "), pt(&g), t(",  O = "), pt(&o)])))
            }
            _ => Err(format!("{f}: wrong number of arguments")),
        }
    })())
}
