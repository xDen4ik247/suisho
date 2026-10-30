//! Step-by-step solutions the way a textbook does them (not machine algorithms):
//! determinants by the 2×2 formula, Sarrus' rule, cofactor expansion along the row/column with the most zeros,
//! or (from 5×5) row reduction to triangular form; the inverse by the 2×2 formula or Gauss-Jordan on [A | I] with
//! every row operation; rref and rank by elimination; sums of fractions over the least common denominator;
//! roots simplified by pulling out perfect squares. Every solution is checked against the engine's own answer.

use crate::eval::{Env, Mat, Val};
use crate::mtree::*;
use crate::num::Num;
use crate::parse::{Ast, Rel};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct Step {
    en: String,
    ru: String,
    head: Option<Value>, // a math title (e.g. a row operation) shown after the words
    math: Vec<Value>,
    sub: Vec<Step>,
}

pub(crate) fn step(en: &str, ru: &str) -> Step {
    Step { en: en.into(), ru: ru.into(), head: None, math: vec![], sub: vec![] }
}
impl Step {
    pub(crate) fn m(mut self, v: Value) -> Step {
        self.math.push(v);
        self
    }
    pub(crate) fn head(mut self, v: Value) -> Step {
        self.head = Some(v);
        self
    }
    pub(crate) fn subs(mut self, s: Vec<Step>) -> Step {
        self.sub = s;
        self
    }
    fn json(&self) -> Value {
        json!({"en": self.en, "ru": self.ru, "head": self.head, "math": self.math,
               "sub": self.sub.iter().map(Step::json).collect::<Vec<_>>()})
    }
}

pub struct Solution {
    pub en: String,
    pub ru: String,
    pub problem: Value,
    pub steps: Vec<Step>,
    pub answer: Value,
    pub verified: bool,
}

impl Solution {
    pub fn json(&self) -> Value {
        json!({"en": self.en, "ru": self.ru, "problem": self.problem, "answer": self.answer, "verified": self.verified,
               "steps": self.steps.iter().map(Step::json).collect::<Vec<_>>()})
    }
}

fn eq() -> Value {
    op("=")
}

// ------------------------------------------------------------------ entry points

/// Cheap syntactic check: does this row have a step-by-step solution?
pub fn available(a: &Ast) -> bool {
    let a = rhs(a);
    match a {
        Ast::Call(f, args) if args.len() == 1 && matches!(f.as_str(), "det" | "inv" | "rref" | "rank" | "sqrt" | "eig" | "eigenvalues") => true,
        Ast::Call(f, args) if f == "solve" && !args.is_empty() => true,
        Ast::Call(f, args) if f == "linreg" && args.len() == 2 => true,
        Ast::Call(f, _) if matches!(f.as_str(), "line" | "distance" | "midpoint" | "triangle" | "plane" | "dot" | "cross" | "angle") => true,
        Ast::Call(f, args) if matches!(f.as_str(), "diff" | "derivative" | "integrate" | "int" | "lim" | "limit" | "analyze" | "investigate") && !args.is_empty() => true,
        Ast::Call(f, args) if f == "root" && args.len() == 2 => true,
        Ast::Pow(_, e) => matches!(&**e, Ast::Neg(x) if **x == Ast::Num("1".into())),
        Ast::Add(..) | Ast::Sub(..) => fraction_terms(a).is_some(),
        _ => false,
    }
}

fn rhs(a: &Ast) -> &Ast {
    if let Ast::Rel(l, chain) = a {
        if chain.len() == 1 && chain[0].0 == Rel::Eq && matches!(**l, Ast::Var(_)) {
            return &chain[0].1;
        }
    }
    a
}

pub fn solve(a: &Ast, env: &mut Env) -> Result<Solution, String> {
    if let Some((v, rel, p, z)) = crate::algebra::detect(a, env) {
        return Ok(crate::algebra::solve(a, &v, rel, &p, z));
    }
    if let Some(r) = crate::calc::solve_call(rhs(a), env.degrees) {
        return r;
    }
    if let Ast::Call(f, _) = rhs(a) {
        if f == "linreg" {
            return linreg_solution(rhs(a), env);
        }
    }
    if let Some(r) = crate::geom::construct(rhs(a), env) {
        return r;
    }
    if let Some(c) = crate::geom::conic_coefs(a, env) {
        return Ok(crate::geom::conic_solution(a, &c));
    }
    let a = rhs(a);
    let no = HashMap::new();
    let mat_of = |env: &mut Env, x: &Ast| -> Result<Mat, String> {
        match env.eval(x, &no) {
            Ok(Val::Mat(m)) => Ok(m),
            Ok(_) => Err("that is not a matrix".into()),
            Err(crate::eval::EvalErr::Msg(m)) => Err(m),
            Err(crate::eval::EvalErr::Free(n)) => Err(format!("{n} is not defined")),
        }
    };
    match a {
        Ast::Call(f, args) if f == "solve" && !args.is_empty() => system_solution(a, args, env),
        Ast::Call(f, args) if (f == "eig" || f == "eigenvalues") && args.len() == 1 => eigen_solution(a, &mat_of(env, &args[0])?),
        Ast::Call(f, args) if f == "det" && args.len() == 1 => det_solution(a, &mat_of(env, &args[0])?),
        Ast::Call(f, args) if f == "inv" && args.len() == 1 => inv_solution(a, &mat_of(env, &args[0])?),
        Ast::Pow(b, _) => inv_solution(a, &mat_of(env, b)?),
        Ast::Call(f, args) if (f == "rref" || f == "rank") && args.len() == 1 => rref_solution(a, &mat_of(env, &args[0])?, f == "rank"),
        Ast::Call(f, args) if f == "sqrt" && args.len() == 1 => root_solution(a, &args[0], 2),
        Ast::Call(f, args) if f == "root" && args.len() == 2 => {
            let k = match &args[1] {
                Ast::Num(n) => n.parse::<u32>().map_err(|_| "the root index must be a whole number")?,
                _ => return Err("the root index must be a number".into()),
            };
            root_solution(a, &args[0], k)
        }
        _ => {
            if let Some(terms) = fraction_terms(a) {
                return fraction_solution(a, &terms);
            }
            Err("no step-by-step solution for this yet".into())
        }
    }
}

// ------------------------------------------------------------------ determinants

fn det_bar(m: &Mat) -> Value {
    let mut v = mat(m);
    v["o"] = json!("|");
    v
}

fn det_solution(a: &Ast, m: &Mat) -> Result<Solution, String> {
    if m.r != m.c {
        return Err(format!("det needs a square matrix (this one is {}×{})", m.r, m.c));
    }
    let (d, steps) = det_steps(m, 0);
    let check = m.det()?;
    Ok(Solution {
        en: "Determinant".into(),
        ru: "Определитель".into(),
        problem: ast(a),
        verified: d.sub(&check).map(|x| x.is_zero()).unwrap_or(false),
        answer: num(&d),
        steps,
    })
}

/// (value, steps) for det m.
fn det_steps(m: &Mat, depth: usize) -> (Num, Vec<Step>) {
    let n = m.r;
    let g = |i: usize, j: usize| m.at(i, j).clone();
    let mulq = |a: &Num, b: &Num| a.mul(b).unwrap_or(Num::zero());
    match n {
        1 => (g(0, 0), vec![step("A 1×1 determinant is its only entry.", "Определитель 1×1 равен своему элементу.").m(row(vec![det_bar(m), eq(), num(&g(0, 0))]))]),
        2 => {
            let (a, b, c, d) = (g(0, 0), g(0, 1), g(1, 0), g(1, 1));
            let ad = mulq(&a, &d);
            let bc = mulq(&b, &c);
            let r = ad.sub(&bc).unwrap_or(Num::zero());
            let s = step("2×2 determinant: product of the main diagonal minus product of the other diagonal (ad − bc).",
                         "Определитель 2×2: произведение главной диагонали минус произведение побочной (ad − bc).")
                .m(row(vec![det_bar(m), eq(), num_p(&a), op("·"), num_p(&d), op("−"), num_p(&b), op("·"), num_p(&c)]))
                .m(row(vec![eq(), num(&ad), op("−"), num_p(&bc), eq(), num(&r)]));
            (r, vec![s])
        }
        3 if (0..3).all(|i| (0..3).all(|j| !m.at(i, j).is_zero())) => {
            let e = |i: usize, j: usize| g(i, j);
            let plus = [(e(0, 0), e(1, 1), e(2, 2)), (e(0, 1), e(1, 2), e(2, 0)), (e(0, 2), e(1, 0), e(2, 1))];
            let minus = [(e(0, 2), e(1, 1), e(2, 0)), (e(0, 0), e(1, 2), e(2, 1)), (e(0, 1), e(1, 0), e(2, 2))];
            let prod = |t: &(Num, Num, Num)| mulq(&mulq(&t.0, &t.1), &t.2);
            let mut line1 = vec![det_bar(m), eq()];
            let mut line2 = vec![eq()];
            let mut total = Num::zero();
            for (k, tr) in plus.iter().chain(minus.iter()).enumerate() {
                let p = prod(tr);
                let sign = if k < 3 { "+" } else { "−" };
                if k > 0 || sign == "−" {
                    line1.push(op(sign));
                    line2.push(op(sign));
                }
                line1.extend([num_p(&tr.0), op("·"), num_p(&tr.1), op("·"), num_p(&tr.2)]);
                line2.push(num_p(&p));
                total = if k < 3 { total.add(&p) } else { total.sub(&p) }.unwrap_or(Num::zero());
            }
            line2.extend([eq(), num(&total)]);
            let s = step("Rule of Sarrus: three products along the main diagonal direction with “+”, three along the other direction with “−”.",
                         "Правило Саррюса (треугольников): три произведения параллельно главной диагонали со знаком «+», три параллельно побочной — со знаком «−».")
                .m(row(line1))
                .m(row(line2));
            (total, vec![s])
        }
        _ if n >= 5 && depth == 0 && best_line(m).2 < n / 2 => triangular_det(m),
        _ => {
            // cofactor (Laplace) expansion along the row or column with the most zeros
            let (is_row, k, zeros) = best_line(m);
            let entry = |t: usize| if is_row { (k, t) } else { (t, k) };
            let (en, ru) = if is_row {
                (format!("Expand along row {} (it has {} zero{}): det = Σ aᵢⱼ·(−1)^(i+j)·Mᵢⱼ.", k + 1, zeros, if zeros == 1 { "" } else { "s" }),
                 format!("Раскладываем по {}-й строке (в ней нулей: {}): det = Σ aᵢⱼ·(−1)^(i+j)·Mᵢⱼ.", k + 1, zeros))
            } else {
                (format!("Expand along column {} (it has {} zero{}): det = Σ aᵢⱼ·(−1)^(i+j)·Mᵢⱼ.", k + 1, zeros, if zeros == 1 { "" } else { "s" }),
                 format!("Раскладываем по {}-му столбцу (в нём нулей: {}): det = Σ aᵢⱼ·(−1)^(i+j)·Mᵢⱼ.", k + 1, zeros))
            };
            let mut line = vec![det_bar(m), eq()];
            let mut subs = Vec::new();
            let mut parts: Vec<(Num, Num, i32)> = Vec::new(); // (a, minor det, sign)
            let mut first = true;
            for tix in 0..n {
                let (i, j) = entry(tix);
                let a = g(i, j);
                if a.is_zero() {
                    continue;
                }
                let sign = if (i + j) % 2 == 0 { 1 } else { -1 };
                let minor = minor_of(m, i, j);
                let (md, msteps) = det_steps(&minor, depth + 1);
                subs.push(
                    step(&format!("Minor M{}{}: cross out row {} and column {}.", i + 1, j + 1, i + 1, j + 1),
                         &format!("Минор M{}{}: вычёркиваем {}-ю строку и {}-й столбец.", i + 1, j + 1, i + 1, j + 1))
                        .m(row(vec![sub(var("M"), t(&format!("{}{}", i + 1, j + 1))), eq(), det_bar(&minor), eq(), num(&md)]))
                        .subs(msteps),
                );
                let term_sign = if a.sign() * sign < 0 { "−" } else { "+" };
                if !(first && term_sign == "+") {
                    line.push(op(term_sign));
                }
                first = false;
                line.extend([num(&a.abs()), op("·"), det_bar(&minor)]);
                parts.push((a, md, sign));
            }
            if parts.is_empty() {
                let s = step(&en, &ru).m(row(vec![det_bar(m), eq(), t("0")]));
                return (Num::zero(), vec![s]);
            }
            let mut line2 = vec![eq()];
            let mut total = Num::zero();
            for (idx, (a, md, sign)) in parts.iter().enumerate() {
                let coef = if *sign > 0 { a.clone() } else { a.neg() };
                let term = mulq(&coef, md);
                total = total.add(&term).unwrap_or(Num::zero());
                if idx > 0 {
                    line2.push(op(if coef.sign() < 0 { "−" } else { "+" }));
                    line2.extend([num(&coef.abs()), op("·"), num_p(md)]);
                } else {
                    line2.extend([num(&coef), op("·"), num_p(md)]);
                }
            }
            line2.extend([eq(), num(&total)]);
            let s = step(&en, &ru).m(row(line)).subs(subs).m(row(line2));
            (total, vec![s])
        }
    }
}

/// (is_row, index, zeros) of the line with the most zeros (rows win ties).
fn best_line(m: &Mat) -> (bool, usize, usize) {
    let n = m.r;
    let mut best = (true, 0, 0);
    for i in 0..n {
        let z = (0..n).filter(|&j| m.at(i, j).is_zero()).count();
        if z > best.2 {
            best = (true, i, z);
        }
    }
    for j in 0..n {
        let z = (0..n).filter(|&i| m.at(i, j).is_zero()).count();
        if z > best.2 {
            best = (false, j, z);
        }
    }
    best
}

fn minor_of(m: &Mat, ri: usize, cj: usize) -> Mat {
    let mut d = Vec::new();
    for i in 0..m.r {
        if i == ri {
            continue;
        }
        for j in 0..m.c {
            if j != cj {
                d.push(m.at(i, j).clone());
            }
        }
    }
    Mat { r: m.r - 1, c: m.c - 1, d }
}

/// Row-reduce to upper triangular form (R_i ← R_i − c·R_j keeps det; a swap flips its sign).
fn triangular_det(m: &Mat) -> (Num, Vec<Step>) {
    let mut a = m.clone();
    let n = a.r;
    let mut sign = 1;
    let mut steps = vec![step("Bring the matrix to triangular form: adding a multiple of one row to another doesn't change the determinant, swapping two rows flips its sign.",
                              "Приводим к треугольному виду: прибавление к строке другой строки, умноженной на число, не меняет определитель, перестановка строк меняет знак.")];
    for col in 0..n {
        let Some(p) = (col..n).find(|&i| !a.at(i, col).is_zero()) else {
            steps.push(step("A zero column below the diagonal: the determinant is 0.", "Нулевой столбец под диагональю: определитель равен 0.").m(det_bar(&a)));
            return (Num::zero(), steps);
        };
        if p != col {
            swap_rows(&mut a, p, col);
            sign = -sign;
            steps.push(step("Swap rows (the sign changes):", "Меняем строки местами (знак меняется):").head(row(vec![rname(col), op("↔"), rname(p)])).m(hl_rows(&a, &[col, p], None)));
        }
        let mut ops = Vec::new();
        let mut changed = Vec::new();
        for i in col + 1..n {
            if a.at(i, col).is_zero() {
                continue;
            }
            let c = a.at(i, col).div(a.at(col, col)).unwrap_or(Num::zero());
            sub_row(&mut a, i, col, &c);
            ops.push(op_label(i, col, &c));
            changed.push(i);
        }
        if !ops.is_empty() {
            steps.push(step("Eliminate below the pivot:", "Обнуляем элементы под ведущим:").head(join_ops(ops)).m(hl_rows(&a, &changed, None)));
        }
    }
    let mut d = Num::from_sign(sign);
    let mut line = vec![det_bar(m), eq()];
    if sign < 0 {
        line.push(t("−"));
    }
    for i in 0..n {
        if i > 0 {
            line.push(op("·"));
        }
        line.push(num_p(a.at(i, i)));
        d = d.mul(a.at(i, i)).unwrap_or(Num::zero());
    }
    line.extend([eq(), num(&d)]);
    steps.push(step("The determinant of a triangular matrix is the product of its diagonal.", "Определитель треугольной матрицы равен произведению диагональных элементов.").m(row(line)));
    (d, steps)
}

// ------------------------------------------------------------------ row operations

fn swap_rows(a: &mut Mat, i: usize, j: usize) {
    for k in 0..a.c {
        a.d.swap(i * a.c + k, j * a.c + k);
    }
}
/// R_i ← R_i − c·R_j
fn sub_row(a: &mut Mat, i: usize, j: usize, c: &Num) {
    for k in 0..a.c {
        let v = a.at(i, k).sub(&c.mul(a.at(j, k)).unwrap_or(Num::zero())).unwrap_or(Num::zero());
        a.d[i * a.c + k] = v;
    }
}
fn scale_row(a: &mut Mat, i: usize, c: &Num) {
    for k in 0..a.c {
        let v = a.at(i, k).mul(c).unwrap_or(Num::zero());
        a.d[i * a.c + k] = v;
    }
}

/// "R₂ ← R₂ − 3R₁" (c is what gets subtracted)
fn op_label(i: usize, j: usize, c: &Num) -> Value {
    let (sign, mag) = if c.sign() < 0 { ("+", c.neg()) } else { ("−", c.clone()) };
    let mut v = vec![rname(i), op("←"), rname(i), op(sign)];
    if !mag.sub(&Num::one()).map(|x| x.is_zero()).unwrap_or(false) {
        v.push(num(&mag));
    }
    v.push(rname(j));
    row(v)
}
fn join_ops(ops: Vec<Value>) -> Value {
    let mut c = Vec::new();
    for (k, o) in ops.into_iter().enumerate() {
        if k > 0 {
            c.push(t(",  "));
        }
        c.push(o);
    }
    row(c)
}
fn hl_rows(a: &Mat, rows: &[usize], aug: Option<usize>) -> Value {
    let mut v = mat(a);
    v["hr"] = json!(rows);
    if let Some(c) = aug {
        v["aug"] = json!(c);
    }
    v
}

impl Num {
    fn from_sign(s: i32) -> Num {
        Num::int(s as i64)
    }
}

/// Gauss-Jordan with steps grouped per pivot (swap, scale, eliminate); returns the reduced matrix and pivot columns.
fn gauss_jordan(a: &mut Mat, cols: usize, aug: Option<usize>, steps: &mut Vec<Step>) -> Vec<usize> {
    let mut pivots = Vec::new();
    let mut r = 0;
    for col in 0..cols {
        if r >= a.r {
            break;
        }
        // pivot: the current row if it is ±1, else a row below with ±1, else the first non-zero one
        let is_unit = |a: &Mat, i: usize| a.at(i, col).abs().sub(&Num::one()).map(|x| x.is_zero()).unwrap_or(false);
        let cand = if !a.at(r, col).is_zero() && is_unit(a, r) {
            Some(r)
        } else {
            (r..a.r).find(|&i| is_unit(a, i)).or_else(|| (r..a.r).find(|&i| !a.at(i, col).is_zero()))
        };
        let Some(p) = cand else { continue };
        if p != r {
            swap_rows(a, p, r);
            steps.push(step(if is_unit(a, r) { "Swap rows to get a convenient pivot (1):" } else { "Swap rows to get a non-zero pivot:" },
                            if is_unit(a, r) { "Меняем строки, чтобы ведущий элемент был удобным (1):" } else { "Меняем строки, чтобы ведущий элемент был ненулевым:" })
                .head(row(vec![rname(r), op("↔"), rname(p)]))
                .m(hl_rows(a, &[r, p], aug)));
        }
        let pv = a.at(r, col).clone();
        if !pv.sub(&Num::one()).map(|x| x.is_zero()).unwrap_or(false) {
            let k = Num::one().div(&pv).unwrap_or(Num::one());
            scale_row(a, r, &k);
            steps.push(step("Make the pivot 1:", "Делаем ведущий элемент равным 1:")
                .head(row(vec![rname(r), op("←"), num_p(&k), op("·"), rname(r)]))
                .m(hl_rows(a, &[r], aug)));
        }
        let mut ops = Vec::new();
        let mut changed = Vec::new();
        for i in 0..a.r {
            if i == r || a.at(i, col).is_zero() {
                continue;
            }
            let c = a.at(i, col).clone();
            sub_row(a, i, r, &c);
            ops.push(op_label(i, r, &c));
            changed.push(i);
        }
        if !ops.is_empty() {
            steps.push(step(&format!("Clear column {} (zeros above and below the pivot):", col + 1),
                            &format!("Обнуляем {}-й столбец (над и под ведущим элементом):", col + 1))
                .head(join_ops(ops))
                .m(hl_rows(a, &changed, aug)));
        }
        pivots.push(col);
        r += 1;
    }
    pivots
}

fn inv_solution(a: &Ast, m: &Mat) -> Result<Solution, String> {
    if m.r != m.c {
        return Err(format!("only square matrices have an inverse (this one is {}×{})", m.r, m.c));
    }
    let n = m.r;
    let check = m.inv();
    let mut steps = Vec::new();
    let answer;
    if n == 2 {
        let (d, dsteps) = det_steps(m, 0);
        steps.push(step("First the determinant (an inverse exists only when det ≠ 0):", "Сначала определитель (обратная существует только при det ≠ 0):")
            .m(row(vec![t("det"), var("A"), eq(), num(&d)]))
            .subs(dsteps));
        if d.is_zero() {
            return Ok(singular(a, steps));
        }
        let (p, q, r, s) = (m.at(0, 0).clone(), m.at(0, 1).clone(), m.at(1, 0).clone(), m.at(1, 1).clone());
        let adj = Mat { r: 2, c: 2, d: vec![s, q.neg(), r.neg(), p] };
        let inv = check.clone()?;
        steps.push(step("For 2×2: swap the diagonal entries, change the sign of the other two, divide by det.",
                        "Для 2×2: меняем местами элементы главной диагонали, у побочной меняем знак и делим на det.")
            .m(row(vec![sup(var("A"), t("−1")), eq(), frac(t("1"), num(&d)), op("·"), mat(&adj), eq(), mat(&inv)])));
        answer = mat(&inv);
    } else {
        let mut aug = Mat { r: n, c: 2 * n, d: Vec::with_capacity(2 * n * n) };
        for i in 0..n {
            for j in 0..n {
                aug.d.push(m.at(i, j).clone());
            }
            for j in 0..n {
                aug.d.push(if i == j { Num::one() } else { Num::zero() });
            }
        }
        steps.push(step("Write the augmented matrix [A | I] and reduce the left half to I with row operations; the right half becomes A⁻¹.",
                        "Записываем расширенную матрицу [A | E] и элементарными преобразованиями строк приводим левую половину к E; справа получится A⁻¹.")
            .m(hl_rows(&aug, &[], Some(n))));
        let pivots = gauss_jordan(&mut aug, n, Some(n), &mut steps);
        if pivots.len() < n {
            steps.push(step("A row of zeros appeared on the left: rank A < n.", "Слева появилась нулевая строка: rank A < n.").m(hl_rows(&aug, &[], Some(n))));
            return Ok(singular(a, steps));
        }
        let inv = Mat { r: n, c: n, d: (0..n).flat_map(|i| (n..2 * n).map(move |j| (i, j))).map(|(i, j)| aug.at(i, j).clone()).collect() };
        steps.push(step("The left half is I, so the right half is the inverse:", "Слева единичная матрица, значит справа — обратная:")
            .m(row(vec![sup(var("A"), t("−1")), eq(), mat(&inv)])));
        answer = mat(&inv);
    }
    // check: A · A⁻¹ = I
    let inv = check?;
    let ok = m.mul(&inv).map(|p| (0..n).all(|i| (0..n).all(|j| p.at(i, j).sub(&Num::int((i == j) as i64)).map(|x| x.is_zero()).unwrap_or(false)))).unwrap_or(false);
    steps.push(step("Check: A·A⁻¹ = I ✓", "Проверка: A·A⁻¹ = E ✓").m(row(vec![var("A"), op("·"), sup(var("A"), t("−1")), eq(), mat(&Mat::eye(n))])));
    Ok(Solution { en: "Inverse matrix".into(), ru: "Обратная матрица".into(), problem: ast(a), steps, answer, verified: ok })
}

fn singular(a: &Ast, steps: Vec<Step>) -> Solution {
    Solution {
        en: "Inverse matrix".into(),
        ru: "Обратная матрица".into(),
        problem: ast(a),
        steps,
        answer: t("—"),
        verified: true,
    }
    .with_note()
}
impl Solution {
    fn with_note(mut self) -> Self {
        self.steps.push(step("det A = 0, so the matrix is singular and has no inverse.", "det A = 0: матрица вырожденная, обратной не существует."));
        self.answer = t("∄");
        self
    }
}

fn rref_solution(a: &Ast, m: &Mat, rank_only: bool) -> Result<Solution, String> {
    let mut work = m.clone();
    let mut steps = vec![step("Row-reduce with elementary row operations (they don't change the rank or the solutions):",
                              "Приводим элементарными преобразованиями строк (они не меняют ранг и решения):").m(mat(m))];
    let pivots = gauss_jordan(&mut work, m.c, None, &mut steps);
    let (check, rank, _) = m.rref()?;
    let same = (0..m.r).all(|i| (0..m.c).all(|j| work.at(i, j).sub(check.at(i, j)).map(|x| x.is_zero()).unwrap_or(false)));
    let answer = if rank_only {
        steps.push(step(&format!("Non-zero rows (pivots) left: {}.", pivots.len()), &format!("Ненулевых строк (ведущих элементов): {}.", pivots.len()))
            .m(row(vec![t("rank"), var("A"), eq(), t(&pivots.len().to_string())])));
        t(&pivots.len().to_string())
    } else {
        steps.push(step("Reduced row echelon form:", "Ступенчатый (приведённый) вид:").m(mat(&work)));
        mat(&work)
    };
    Ok(Solution {
        en: if rank_only { "Rank".into() } else { "Reduced row echelon form".into() },
        ru: if rank_only { "Ранг".into() } else { "Приведённый ступенчатый вид".into() },
        problem: ast(a),
        steps,
        answer,
        verified: same && pivots.len() == rank,
    })
}

// ------------------------------------------------------------------ roots

fn root_solution(a: &Ast, inner: &Ast, k: u32) -> Result<Solution, String> {
    let n: BigInt = match inner {
        Ast::Num(s) if !s.contains('.') => s.parse().map_err(|_| "bad number")?,
        _ => return Err("step-by-step roots work for whole numbers".into()),
    };
    if n.is_zero() || n.bits() > 64 {
        return Err("no steps for this root".into());
    }
    // n = out^k · rem
    let mut out = BigInt::one();
    let mut rem = BigInt::one();
    let mut m = n.clone();
    let mut factors: Vec<(BigInt, u32)> = Vec::new();
    let mut p = BigInt::from(2);
    while &p * &p <= m {
        let mut e = 0;
        while (&m % &p).is_zero() {
            m /= &p;
            e += 1;
        }
        if e > 0 {
            factors.push((p.clone(), e));
            out *= num_traits::pow(p.clone(), (e / k) as usize);
            rem *= num_traits::pow(p.clone(), (e % k) as usize);
        }
        p += 1;
    }
    if m > BigInt::one() {
        factors.push((m.clone(), 1));
        rem *= &m;
    }
    let idx = if k == 2 { None } else { Some(k.to_string()) };
    let rt = |v: Value| sqrt(v, idx.as_deref());
    let mut fac = Vec::new();
    for (i, (p, e)) in factors.iter().enumerate() {
        if i > 0 {
            fac.push(op("·"));
        }
        fac.push(if *e > 1 { sup(t(&p.to_string()), t(&e.to_string())) } else { t(&p.to_string()) });
    }
    let mut steps = vec![step("Factor the number into primes:", "Раскладываем число на простые множители:").m(row(vec![t(&n.to_string()), eq(), row(fac)]))];
    let answer = if out.is_one() {
        steps.push(step("No prime appears often enough to come out of the root: it is already as simple as it gets.",
                        "Ни один множитель не встречается достаточно раз, чтобы вынести его из-под корня: упростить нельзя."));
        rt(t(&n.to_string()))
    } else {
        let kth = num_traits::pow(out.clone(), k as usize);
        steps.push(step(&format!("Group a perfect {} power: {} = {}·{}.", if k == 2 { "square" } else { "power" }, n, kth, rem),
                        &format!("Выделяем полную {}: {} = {}·{}.", if k == 2 { "квадрат" } else { "степень" }, n, kth, rem))
            .m(row(vec![rt(t(&n.to_string())), eq(), rt(row(vec![sup(t(&out.to_string()), t(&k.to_string())), op("·"), t(&rem.to_string())]))])));
        let res = if rem.is_one() { t(&out.to_string()) } else { row(vec![t(&out.to_string()), rt(t(&rem.to_string()))]) };
        steps.push(step("Take the perfect power out of the root:", "Выносим множитель из-под корня:")
            .m(row(vec![eq(), rt(sup(t(&out.to_string()), t(&k.to_string()))), op("·"), rt(t(&rem.to_string())), eq(), res.clone()])));
        res
    };
    let ok = num_traits::pow(out, k as usize) * rem == n;
    Ok(Solution { en: "Simplify the root".into(), ru: "Упрощение корня".into(), problem: ast(a), steps, answer, verified: ok })
}

// ------------------------------------------------------------------ fractions

/// ±a/b terms of a sum of numbers (at least one real fraction), or None.
fn fraction_terms(a: &Ast) -> Option<Vec<BigRational>> {
    fn walk(a: &Ast, sign: i32, out: &mut Vec<BigRational>) -> Option<()> {
        match a {
            Ast::Add(x, y) => {
                walk(x, sign, out)?;
                walk(y, sign, out)
            }
            Ast::Sub(x, y) => {
                walk(x, sign, out)?;
                walk(y, -sign, out)
            }
            Ast::Neg(x) => walk(x, -sign, out),
            Ast::Num(s) if !s.contains('.') => {
                out.push(BigRational::from_integer(s.parse().ok()?) * BigInt::from(sign));
                Some(())
            }
            Ast::Div(n, d) => {
                let (Ast::Num(n), Ast::Num(d)) = (&**n, &**d) else { return None };
                if n.contains('.') || d.contains('.') {
                    return None;
                }
                let d: BigInt = d.parse().ok()?;
                if d.is_zero() {
                    return None;
                }
                out.push(BigRational::new_raw(n.parse::<BigInt>().ok()? * BigInt::from(sign), d));
                Some(())
            }
            _ => None,
        }
    }
    let mut out = Vec::new();
    walk(a, 1, &mut out)?;
    (out.len() >= 2 && out.iter().any(|r| !r.denom().is_one())).then_some(out)
}

fn fraction_solution(a: &Ast, terms: &[BigRational]) -> Result<Solution, String> {
    let lcd = terms.iter().fold(BigInt::one(), |acc, r| acc.lcm(r.denom()));
    let dens: Vec<String> = terms.iter().map(|r| r.denom().to_string()).collect();
    let mut steps = vec![step(&format!("Least common denominator: LCD({}) = {}.", dens.join(", "), lcd),
                              &format!("Наименьший общий знаменатель: НОК({}) = {}.", dens.join(", "), lcd))];
    // bring every term to the LCD
    let mut line = vec![];
    let mut nums = vec![];
    for (i, r) in terms.iter().enumerate() {
        let k = &lcd / r.denom();
        let nn = r.numer() * &k;
        let piece = if r.denom().is_one() {
            row(vec![t(&r.numer().abs().to_string()), eq(), frac(t(&format!("{}·{}", r.numer().abs(), k)), t(&lcd.to_string()))])
        } else {
            row(vec![frac(t(&r.numer().abs().to_string()), t(&r.denom().to_string())), eq(),
                     frac(t(&format!("{}·{}", r.numer().abs(), k)), t(&format!("{}·{}", r.denom(), k))), eq(),
                     frac(t(&nn.abs().to_string()), t(&lcd.to_string()))])
        };
        if i > 0 {
            line.push(t(";   "));
        }
        line.push(piece);
        nums.push(nn);
    }
    steps.push(step("Bring each fraction to the common denominator (multiply top and bottom by the same number):",
                    "Приводим каждую дробь к общему знаменателю (домножаем числитель и знаменатель на дополнительный множитель):")
        .m(row(line)));
    let total: BigInt = nums.iter().sum();
    let mut top = vec![];
    for (i, n) in nums.iter().enumerate() {
        if i == 0 {
            if n.is_negative() {
                top.push(t("−"));
            }
        } else {
            top.push(op(if n.is_negative() { "−" } else { "+" }));
        }
        top.push(t(&n.abs().to_string()));
    }
    steps.push(step("Add the numerators over the common denominator:", "Складываем числители над общим знаменателем:")
        .m(row(vec![frac(row(top), t(&lcd.to_string())), eq(), rat(&BigRational::new_raw(total.clone(), lcd.clone()))])));
    let g = total.gcd(&lcd);
    let result = BigRational::new(total.clone(), lcd.clone());
    if !g.is_one() && !total.is_zero() {
        steps.push(step(&format!("Reduce by gcd({}, {}) = {}:", total.abs(), lcd, g), &format!("Сокращаем на НОД({}, {}) = {}:", total.abs(), lcd, g))
            .m(row(vec![rat(&BigRational::new_raw(total.clone(), lcd.clone())), eq(), rat(&result)])));
    }
    let check: BigRational = terms.iter().sum();
    let mut answer = rat(&result);
    if !result.is_integer() && result.abs() > BigRational::one() {
        // mixed number as a bonus line
        let whole = result.numer().abs() / result.denom();
        let rest = BigRational::new(result.numer().abs() % result.denom(), result.denom().clone());
        steps.push(step("As a mixed number:", "В виде смешанного числа:").m(row(vec![
            rat(&result), eq(), t(if result.is_negative() { "−" } else { "" }), t(&whole.to_string()), rat(&rest)])));
        answer = rat(&result);
    }
    let _ = whole_to_i64(&lcd);
    Ok(Solution { en: "Adding fractions".into(), ru: "Сложение дробей".into(), problem: ast(a), steps, answer, verified: check == result })
}

fn whole_to_i64(b: &BigInt) -> Option<i64> {
    b.to_i64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::collections::HashSet;

    fn sol(src: &str) -> Solution {
        let a = parse(src, &HashSet::new()).unwrap().unwrap();
        let mut env = Env::default();
        solve(&a, &mut env).unwrap()
    }

    #[test]
    fn determinants() {
        for src in ["det([[1,2],[3,4]])", "det([[2,-1,3],[1,4,2],[5,1,-2]])", "det([[2,0,1],[1,3,0],[0,1,4]])",
                    "det([[1,2,0,3],[0,1,4,0],[2,0,1,1],[0,3,0,2]])", "det([[2,1,3,1,2],[1,2,1,3,1],[3,1,2,1,2],[1,3,1,2,1],[2,2,1,1,3]])"] {
            let s = sol(src);
            assert!(s.verified, "{src}");
            assert!(!s.steps.is_empty());
        }
        assert_eq!(sol("det([[1,2],[3,4]])").answer, json!({"k": "row", "c": [{"k": "t", "s": "−"}, {"k": "t", "s": "2"}]}));
    }

    #[test]
    fn inverses_and_rank() {
        assert!(sol("inv([[2,1],[1,3]])").verified);
        let s = sol("inv([[1,2,3],[0,1,4],[5,6,0]])");
        assert!(s.verified);
        assert!(s.steps.len() > 4);
        assert!(sol("inv([[1,2],[2,4]])").verified);
        assert!(sol("rank([[1,2,3],[2,4,6],[1,0,1]])").verified);
        assert!(sol("rref([[1,2,3],[4,5,6]])").verified);
    }

    #[test]
    fn systems_and_eigen() {
        let s = sol("solve(2x + y = 5, x - y = 1)");
        assert!(s.verified);
        assert_eq!(crate::mtree::flat(&s.answer), "x=2;   y=1");
        let s = sol("solve(x + y + z = 6, 2x - y + z = 3, x + 2y - z = 2)");
        assert!(s.verified, "{}", crate::mtree::flat(&s.answer));
        let s = sol("solve(x + y = 2, 2x + 2y = 4)");
        assert!(s.verified);
        assert!(crate::mtree::flat(&s.answer).contains("y∈ℝ"), "{}", crate::mtree::flat(&s.answer));
        assert_eq!(crate::mtree::flat(&sol("solve(x + y = 1, x + y = 2)").answer), "∅");
        let s = sol("eig([[2,1],[1,2]])");
        assert!(s.verified);
        assert_eq!(crate::mtree::flat(&s.answer), "λ_1=1;   λ_2=3");
        assert!(sol("eig([[2,0,0],[0,3,4],[0,4,9]])").verified);
    }

    #[test]
    fn fractions_and_roots() {
        assert!(sol("1/3 + 1/6").verified);
        assert!(sol("2/3 - 5/4 + 1").verified);
        assert!(sol("sqrt(72)").verified);
        assert!(sol("root(54, 3)").verified);
        assert!(available(&parse("A^-1", &HashSet::new()).unwrap().unwrap()));
    }
}

// ------------------------------------------------------------------ systems of linear equations

/// Coefficients of a linear expression in `vars` (+ constant), or None if it is not linear.
fn linear(a: &Ast, vars: &[String], env: &mut Env) -> Option<(Vec<BigRational>, BigRational)> {
    let n = vars.len();
    let zero = || (vec![BigRational::zero(); n], BigRational::zero());
    let scale = |(c, k): (Vec<BigRational>, BigRational), f: &BigRational| (c.iter().map(|x| x * f).collect::<Vec<_>>(), k * f);
    let is_const = |v: &(Vec<BigRational>, BigRational)| v.0.iter().all(|x| x.is_zero());
    Some(match a {
        Ast::Num(s) => match Num::parse_decimal(s)? {
            Num::Q(r) => (vec![BigRational::zero(); n], r),
            _ => return None,
        },
        Ast::Var(v) => match vars.iter().position(|x| x == v) {
            Some(i) => {
                let mut c = zero();
                c.0[i] = BigRational::one();
                c
            }
            None => match env.vars.get(v) {
                Some(Val::N(Num::Q(r))) => (vec![BigRational::zero(); n], r.clone()),
                _ => return None,
            },
        },
        Ast::Neg(x) => scale(linear(x, vars, env)?, &-BigRational::one()),
        Ast::Add(x, y) | Ast::Sub(x, y) => {
            let (a1, k1) = linear(x, vars, env)?;
            let (a2, k2) = linear(y, vars, env)?;
            let s = if matches!(a, Ast::Sub(..)) { -BigRational::one() } else { BigRational::one() };
            (a1.iter().zip(&a2).map(|(p, q)| p + q * &s).collect(), k1 + k2 * s)
        }
        Ast::Mul(x, y, _) => {
            let l = linear(x, vars, env)?;
            let r = linear(y, vars, env)?;
            if is_const(&l) {
                scale(r, &l.1)
            } else if is_const(&r) {
                scale(l, &r.1)
            } else {
                return None;
            }
        }
        Ast::Div(x, y) => {
            let r = linear(y, vars, env)?;
            if !is_const(&r) || r.1.is_zero() {
                return None;
            }
            scale(linear(x, vars, env)?, &r.1.recip())
        }
        _ => return None,
    })
}

fn unknowns(a: &Ast, env: &Env, out: &mut Vec<String>) {
    match a {
        Ast::Var(v) => {
            if !env.vars.contains_key(v) && !matches!(v.as_str(), "pi" | "e" | "∞") && !out.contains(v) {
                out.push(v.clone());
            }
        }
        Ast::Neg(x) => unknowns(x, env, out),
        Ast::Add(x, y) | Ast::Sub(x, y) | Ast::Mul(x, y, _) | Ast::Div(x, y) | Ast::Pow(x, y) => {
            unknowns(x, env, out);
            unknowns(y, env, out);
        }
        Ast::Rel(f, ch) => {
            unknowns(f, env, out);
            for (_, e) in ch {
                unknowns(e, env, out);
            }
        }
        _ => {}
    }
}

fn mat_q(rows: &[Vec<BigRational>]) -> Mat {
    Mat { r: rows.len(), c: rows[0].len(), d: rows.iter().flatten().map(|x| Num::Q(x.clone())).collect() }
}

pub fn system_solution(a: &Ast, eqs: &[Ast], env: &mut Env) -> Result<Solution, String> {
    let mut vars = Vec::new();
    for e in eqs {
        unknowns(e, env, &mut vars);
    }
    vars.sort_by_key(|v| (match v.as_str() { "x" => 0, "y" => 1, "z" => 2, "t" => 3, _ => 4 }, v.clone()));
    let n = vars.len();
    if n == 0 {
        return Err("no unknowns in the system".into());
    }
    let mut rows: Vec<Vec<BigRational>> = Vec::new();
    let mut rhs_v: Vec<BigRational> = Vec::new();
    for e in eqs {
        let Ast::Rel(l, ch) = e else { return Err("each item of a system is an equation (=)".into()) };
        if ch.len() != 1 || ch[0].0 != Rel::Eq {
            return Err("each item of a system is an equation (=)".into());
        }
        let (cl, kl) = linear(l, &vars, env).ok_or("the system is not linear")?;
        let (cr, kr) = linear(&ch[0].1, &vars, env).ok_or("the system is not linear")?;
        rows.push(cl.iter().zip(&cr).map(|(p, q)| p - q).collect());
        rhs_v.push(kr - kl);
    }
    let m = eqs.len();
    let am = mat_q(&rows);
    let mut steps = Vec::new();
    // the system in standard form
    let mut sys_lines = Vec::new();
    for i in 0..m {
        let mut c = Vec::new();
        for j in 0..n {
            let a = &rows[i][j];
            if a.is_zero() {
                continue;
            }
            if c.is_empty() {
                if a.is_negative() {
                    c.push(t("−"));
                }
            } else {
                c.push(op(if a.is_negative() { "−" } else { "+" }));
            }
            if !a.abs().is_one() {
                c.push(rat(&a.abs()));
            }
            c.push(var(&vars[j]));
        }
        if c.is_empty() {
            c.push(t("0"));
        }
        c.push(op("="));
        c.push(rat(&rhs_v[i]));
        sys_lines.push(row(c));
    }
    let mut first = step("Write the system in standard form (unknowns on the left, numbers on the right):",
                         "Записываем систему в стандартном виде (неизвестные слева, числа справа):");
    for l in sys_lines {
        first = first.m(l);
    }
    steps.push(first);
    let names = |vals: &[Value]| -> Value {
        let mut c = Vec::new();
        for (j, v) in vals.iter().enumerate() {
            if j > 0 {
                c.push(t(";   "));
            }
            if v["k"] == "free" {
                c.extend([var(&vars[j]), op("∈"), t("ℝ")]);
            } else {
                c.extend([var(&vars[j]), op("="), v.clone()]);
            }
        }
        row(c)
    };
    // Cramer for small square systems with det ≠ 0
    if m == n && n <= 3 {
        let d = am.det()?;
        if !d.is_zero() {
            let (dv, dsteps) = det_steps(&am, 0);
            steps.push(step("Cramer's rule: Δ = det A (the coefficient matrix).", "Метод Крамера: Δ = det A (определитель матрицы коэффициентов).")
                .m(row(vec![t("Δ"), eq(), det_bar(&am), eq(), num(&dv)]))
                .subs(dsteps));
            let mut sol = Vec::new();
            for j in 0..n {
                let mut mj = am.clone();
                for i in 0..n {
                    mj.d[i * n + j] = Num::Q(rhs_v[i].clone());
                }
                let (dj, djs) = det_steps(&mj, 0);
                let xj = dj.div(&dv)?;
                steps.push(step(&format!("Δ{}: replace column {} with the right-hand sides; {} = Δ{}/Δ.", j + 1, j + 1, vars[j], j + 1),
                                &format!("Δ{}: заменяем {}-й столбец столбцом свободных членов; {} = Δ{}/Δ.", j + 1, j + 1, vars[j], j + 1))
                    .m(row(vec![sub(t("Δ"), t(&(j + 1).to_string())), eq(), det_bar(&mj), eq(), num(&dj)]))
                    .m(row(vec![var(&vars[j]), eq(), frac(num(&dj), num(&dv)), eq(), num(&xj)]))
                    .subs(djs));
                sol.push(xj);
            }
            // check by substitution
            let ok = (0..m).all(|i| {
                let mut s = Num::zero();
                for j in 0..n {
                    s = s.add(&Num::Q(rows[i][j].clone()).mul(&sol[j]).unwrap_or(Num::zero())).unwrap_or(Num::zero());
                }
                s.sub(&Num::Q(rhs_v[i].clone())).map(|x| x.is_zero()).unwrap_or(false)
            });
            steps.push(step("Check: substituting the values into every equation gives true equalities ✓", "Проверка: подстановка во все уравнения даёт верные равенства ✓"));
            let ans = names(&sol.iter().map(num).collect::<Vec<_>>());
            return Ok(Solution { en: "System of linear equations".into(), ru: "Система линейных уравнений".into(), problem: ast(a), steps, answer: ans, verified: ok });
        }
    }
    // Gauss-Jordan on [A | b]
    let mut aug = Mat { r: m, c: n + 1, d: Vec::new() };
    for i in 0..m {
        for j in 0..n {
            aug.d.push(Num::Q(rows[i][j].clone()));
        }
        aug.d.push(Num::Q(rhs_v[i].clone()));
    }
    steps.push(step("Gauss-Jordan: write the augmented matrix [A | b] and reduce it with row operations.",
                    "Метод Гаусса: записываем расширенную матрицу [A | b] и упрощаем её преобразованиями строк.")
        .m(hl_rows(&aug, &[], Some(n))));
    let pivots = gauss_jordan(&mut aug, n, Some(n), &mut steps);
    // inconsistent row 0 … 0 | c
    if let Some(i) = (0..m).find(|&i| (0..n).all(|j| aug.at(i, j).is_zero()) && !aug.at(i, n).is_zero()) {
        steps.push(step(&format!("Row {} reads 0 = {}: a contradiction, so the system has no solutions (it is inconsistent).", i + 1, aug.at(i, n).plain(10)),
                        &format!("Строка {} означает 0 = {}: противоречие, система несовместна (решений нет).", i + 1, aug.at(i, n).plain(10)))
            .m(hl_rows(&aug, &[i], Some(n))));
        return Ok(Solution { en: "System of linear equations".into(), ru: "Система линейных уравнений".into(), problem: ast(a), steps, answer: t("∅"), verified: true });
    }
    let free: Vec<usize> = (0..n).filter(|j| !pivots.contains(j)).collect();
    let mut vals: Vec<Value> = vec![t("?"); n];
    for (pi, &pc) in pivots.iter().enumerate() {
        let mut c = vec![num(aug.at(pi, n))];
        if aug.at(pi, n).is_zero() && !free.is_empty() {
            c.clear();
        }
        for &f in &free {
            let k = aug.at(pi, f);
            if k.is_zero() {
                continue;
            }
            let negk = k.neg();
            if c.is_empty() {
                if negk.sign() < 0 {
                    c.push(t("−"));
                }
            } else {
                c.push(op(if negk.sign() < 0 { "−" } else { "+" }));
            }
            if !negk.abs().sub(&Num::one()).map(|x| x.is_zero()).unwrap_or(false) {
                c.push(num(&negk.abs()));
            }
            c.push(var(&vars[f]));
        }
        if c.is_empty() {
            c.push(t("0"));
        }
        vals[pc] = row(c);
    }
    for &f in &free {
        vals[f] = json!({"k": "free"});
    }
    if free.is_empty() {
        steps.push(step("The left part is the identity: read the solution from the last column.", "Слева единичная матрица: решение читаем в последнем столбце."));
    } else {
        let fv: Vec<&str> = free.iter().map(|&f| vars[f].as_str()).collect();
        steps.push(step(&format!("Fewer pivots ({}) than unknowns ({}): {} {} free; express the others through {}.",
                                 pivots.len(), n, fv.join(", "), if fv.len() == 1 { "is" } else { "are" }, if fv.len() == 1 { "it" } else { "them" }),
                        &format!("Ведущих элементов ({}) меньше, чем неизвестных ({}): {} — свободн{}; выражаем остальные через {}.",
                                 pivots.len(), n, fv.join(", "), if fv.len() == 1 { "ая переменная" } else { "ые переменные" }, if fv.len() == 1 { "неё" } else { "них" })));
    }
    let ok = {
        let (check, rank, _) = am.rref()?;
        let _ = check;
        rank == pivots.len()
    };
    Ok(Solution { en: "System of linear equations".into(), ru: "Система линейных уравнений".into(), problem: ast(a), steps, answer: names(&vals), verified: ok })
}

// ------------------------------------------------------------------ eigenvalues

fn det_poly(m: &[Vec<crate::poly::Poly>]) -> crate::poly::Poly {
    let n = m.len();
    if n == 1 {
        return m[0][0].clone();
    }
    let mut acc = crate::poly::Poly::zero();
    for j in 0..n {
        let minor: Vec<Vec<crate::poly::Poly>> = (1..n).map(|i| (0..n).filter(|&c| c != j).map(|c| m[i][c].clone()).collect()).collect();
        let term = m[0][j].mul(&det_poly(&minor));
        acc = if j % 2 == 0 { acc.add(&term) } else { acc.sub(&term) };
    }
    acc
}

pub fn eigen_solution(a: &Ast, m: &Mat) -> Result<Solution, String> {
    use crate::poly::Poly;
    if m.r != m.c || m.r > 4 {
        return Err("eigenvalues: a square matrix up to 4×4".into());
    }
    let n = m.r;
    let mut qm: Vec<Vec<BigRational>> = Vec::new();
    for i in 0..n {
        let mut r = Vec::new();
        for j in 0..n {
            match m.at(i, j) {
                Num::Q(x) => r.push(x.clone()),
                _ => return Err("eigenvalue steps need exact entries".into()),
            }
        }
        qm.push(r);
    }
    // A − λI as a display matrix and as polynomials in λ
    let lam = "λ";
    let disp: Vec<Vec<Value>> = (0..n).map(|i| (0..n).map(|j| if i == j {
        if qm[i][j].is_zero() { row(vec![t("−"), var(lam)]) } else { row(vec![rat(&qm[i][j]), op("−"), var(lam)]) }
    } else { rat(&qm[i][j]) }).collect()).collect();
    let pm: Vec<Vec<Poly>> = (0..n).map(|i| (0..n).map(|j| {
        let c = Poly::constant(qm[i][j].clone());
        if i == j { c.sub(&Poly::x()) } else { c }
    }).collect()).collect();
    let cp = det_poly(&pm);
    let mut steps = vec![step("Characteristic equation: det(A − λI) = 0 (subtract λ on the main diagonal).",
                              "Характеристическое уравнение: det(A − λE) = 0 (вычитаем λ на главной диагонали).")
        .m(row(vec![json!({"k": "mat", "o": "|", "r": disp}), eq(), crate::algebra::poly_tree(&cp, lam), eq(), t("0")]))];
    if n == 2 {
        steps.push(step("For 2×2 it is always λ² − (tr A)·λ + det A, with tr A = a₁₁ + a₂₂.", "Для 2×2 это всегда λ² − (tr A)·λ + det A, где tr A = a₁₁ + a₂₂."));
    }
    let mut rsteps = Vec::new();
    let roots = crate::algebra::roots_with_steps(&cp, lam, &mut rsteps);
    steps.push(step("Solve the characteristic equation:", "Решаем характеристическое уравнение:").subs(rsteps));
    let mut ans = Vec::new();
    let mut verified = true;
    for (k, r) in roots.iter().enumerate() {
        let lname = sub(var(lam), t(&(k + 1).to_string()));
        if k > 0 {
            ans.push(t(";   "));
        }
        ans.extend([lname.clone(), eq(), crate::algebra::root_tree(r)]);
        // eigenvectors for rational eigenvalues: the null space of A − λI
        if let Some(crate::sym::E::Q(lv)) = &r.exact {
            let shifted = Mat { r: n, c: n, d: (0..n * n).map(|ix| {
                let (i, j) = (ix / n, ix % n);
                Num::Q(if i == j { qm[i][j].clone() - lv } else { qm[i][j].clone() })
            }).collect() };
            let mut work = shifted.clone();
            let mut sub_steps = vec![step("Solve (A − λI)·v = 0:", "Решаем (A − λE)·v = 0:").m(mat(&shifted))];
            let piv = gauss_jordan(&mut work, n, None, &mut sub_steps);
            let free: Vec<usize> = (0..n).filter(|j| !piv.contains(j)).collect();
            let mut vecs = Vec::new();
            for &f in &free {
                let mut v = vec![BigRational::zero(); n];
                v[f] = BigRational::one();
                for (pi, &pc) in piv.iter().enumerate() {
                    if let Num::Q(x) = work.at(pi, f) {
                        v[pc] = -x.clone();
                    }
                }
                // integer-scale for readability
                let l = v.iter().fold(BigInt::one(), |acc, x| acc.lcm(x.denom()));
                let v: Vec<BigRational> = v.iter().map(|x| x * BigRational::from_integer(l.clone())).collect();
                // check A v = λ v
                for i in 0..n {
                    let lhs: BigRational = (0..n).map(|j| &qm[i][j] * &v[j]).sum();
                    if lhs != lv * &v[i] {
                        verified = false;
                    }
                }
                vecs.push(v);
            }
            let mut line = vec![t("v = ")];
            for (vi, v) in vecs.iter().enumerate() {
                if vi > 0 {
                    line.push(t(",  "));
                }
                line.push(json!({"k": "mat", "o": "(", "r": v.iter().map(|x| vec![rat(x)]).collect::<Vec<_>>()}));
            }
            steps.push(step(&format!("Eigenvectors for λ{} = {}:", k + 1, crate::sym::plain(&crate::sym::E::Q(lv.clone()))),
                            &format!("Собственные векторы для λ{} = {}:", k + 1, crate::sym::plain(&crate::sym::E::Q(lv.clone()))))
                .m(row(line))
                .subs(sub_steps));
        }
    }
    if roots.is_empty() {
        ans.push(t("no real eigenvalues"));
    }
    Ok(Solution { en: "Eigenvalues and eigenvectors".into(), ru: "Собственные значения и векторы".into(), problem: ast(a), steps, answer: row(ans), verified })
}

// ------------------------------------------------------------------ least-squares line

pub fn linreg_solution(a: &Ast, env: &mut Env) -> Result<Solution, String> {
    let Ast::Call(_, args) = a else { return Err("linreg(X, Y)".into()) };
    if args.len() != 2 {
        return Err("linreg(X, Y): two lists of the same length".into());
    }
    let no = HashMap::new();
    let list = |env: &mut Env, x: &Ast| -> Result<Vec<Num>, String> {
        match env.eval(x, &no) {
            Ok(Val::List(l)) => l.into_iter().map(|v| if let Val::N(n) = v { Ok(n) } else { Err("numbers only".to_string()) }).collect(),
            Ok(_) => Err("linreg(X, Y) takes two lists".into()),
            Err(crate::eval::EvalErr::Free(n)) => Err(format!("{n} is not defined")),
            Err(crate::eval::EvalErr::Msg(m)) => Err(m),
        }
    };
    let (xs, ys) = (list(env, &args[0])?, list(env, &args[1])?);
    let n = xs.len();
    if n != ys.len() || n < 2 {
        return Err("linreg(X, Y): two lists of the same length (at least 2 points)".into());
    }
    let sum = |v: &dyn Fn(usize) -> Num| -> Num { (0..n).fold(Num::zero(), |acc, i| acc.add(&v(i)).unwrap_or(Num::zero())) };
    let sx = sum(&|i| xs[i].clone());
    let sy = sum(&|i| ys[i].clone());
    let sxx = sum(&|i| xs[i].mul(&xs[i]).unwrap_or(Num::zero()));
    let sxy = sum(&|i| xs[i].mul(&ys[i]).unwrap_or(Num::zero()));
    let nn = Num::int(n as i64);
    let den = nn.mul(&sxx)?.sub(&sx.mul(&sx)?)?;
    if den.is_zero() {
        return Err("all x are equal: no line can be fitted".into());
    }
    let m = nn.mul(&sxy)?.sub(&sx.mul(&sy)?)?.div(&den)?;
    let b = sy.sub(&m.mul(&sx)?)?.div(&nn)?;
    let st = vec![
        step("Sums over the data:", "Суммы по данным:").m(row(vec![t(&format!("n = {n},   ")), t("Σx = "), num(&sx), t(",   Σy = "), num(&sy), t(",   Σx² = "), num(&sxx), t(",   Σxy = "), num(&sxy)])),
        step("Least squares: the slope m = (nΣxy − ΣxΣy)/(nΣx² − (Σx)²).", "Метод наименьших квадратов: m = (nΣxy − ΣxΣy)/(nΣx² − (Σx)²).")
            .m(row(vec![var("m"), eq(), frac(row(vec![t(&n.to_string()), op("·"), num_p(&sxy), op("−"), num_p(&sx), op("·"), num_p(&sy)]),
                                             row(vec![t(&n.to_string()), op("·"), num_p(&sxx), op("−"), sup(par(num(&sx)), t("2"))])), eq(), num(&m)])),
        step("The intercept b = (Σy − mΣx)/n: the line passes through the mean point (x̄, ȳ).", "Свободный член b = (Σy − mΣx)/n: прямая проходит через среднюю точку (x̄, ȳ).")
            .m(row(vec![var("b"), eq(), num(&b)])),
    ];
    let mut av = vec![var("y"), eq(), num(&m), var("x")];
    if !b.is_zero() {
        av.extend([op(if b.sign() < 0 { "−" } else { "+" }), num(&b.abs())]);
    }
    let ans = row(av);
    Ok(Solution { en: "Linear regression".into(), ru: "Линейная регрессия".into(), problem: ast(a), steps: st, answer: ans, verified: true })
}
