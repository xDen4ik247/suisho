//! Trainer: problems generated backwards so the numbers come out nice (roots chosen first, determinants built
//! from small entries, antiderivatives differentiated…), the answer checked mathematically (any equivalent form
//! counts: 0.5 = 1/2, roots in any order, an antiderivative up to a constant), the full solution on request, and
//! an Elo-like rating per topic that picks the next level so you succeed about 75 % of the time.

use crate::eval::{Env, Val};
use crate::num::Num;
use crate::parse::{parse, Ast};
use crate::sym::{self, E};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const TOPICS: &[(&str, &str, &str)] = &[
    ("fractions", "Fractions", "Дроби"),
    ("det2", "Determinant 2×2", "Определитель 2×2"),
    ("det3", "Determinant 3×3", "Определитель 3×3"),
    ("inverse", "Inverse 2×2", "Обратная 2×2"),
    ("linear", "Linear equations", "Линейные уравнения"),
    ("quadratic", "Quadratic equations", "Квадратные уравнения"),
    ("cubic", "Cubic equations", "Кубические уравнения"),
    ("system", "Systems 2×2", "Системы 2×2"),
    ("derivative", "Derivatives", "Производные"),
    ("integral", "Integrals", "Интегралы"),
];

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// a in [lo, hi]
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    fn nz(&mut self, lo: i64, hi: i64) -> i64 {
        loop {
            let v = self.int(lo, hi);
            if v != 0 {
                return v;
            }
        }
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[(self.next() % v.len() as u64) as usize]
    }
}

pub struct Problem {
    pub pid: u64,
    pub topic: String,
    pub level: i32,
    pub source: String, // the problem as Suisho input (solved with steps on request)
    pub check: Check,
    pub hint_en: String,
    pub hint_ru: String,
}

pub enum Check {
    Number(Num),
    Roots(Vec<Num>),
    Pair(Num, Num),
    Matrix(Vec<Num>),
    Derivative(E),
    Antiderivative(E), // the integrand
}

fn sgn_term(c: i64, body: &str, first: bool) -> String {
    if c == 0 {
        return String::new();
    }
    let mag = c.abs();
    let coef = if mag == 1 && !body.is_empty() { String::new() } else { mag.to_string() };
    let sign = if c < 0 { if first { "-" } else { " - " } } else if first { "" } else { " + " };
    format!("{sign}{coef}{body}")
}

/// a polynomial from integer coefficients (highest power first) as input text in x
fn poly_text(c: &[i64]) -> String {
    let n = c.len() - 1;
    let mut s = String::new();
    for (i, &a) in c.iter().enumerate() {
        let p = n - i;
        let body = match p {
            0 => String::new(),
            1 => "x".into(),
            _ => format!("x^{p}"),
        };
        let t = sgn_term(a, &body, s.is_empty());
        s.push_str(&t);
    }
    if s.is_empty() {
        "0".into()
    } else {
        s
    }
}

pub fn generate(topic: &str, level: i32, rng: &mut Rng, pid: u64) -> Problem {
    let l = level.clamp(1, 5) as i64;
    let mk = |source: String, check: Check, en: &str, ru: &str| Problem {
        pid,
        topic: topic.to_string(),
        level: l as i32,
        source,
        check,
        hint_en: en.into(),
        hint_ru: ru.into(),
    };
    match topic {
        "fractions" => {
            let dens = [2, 3, 4, 5, 6, 8, 9, 10, 12, 15];
            let k = if l >= 4 { 3 } else { 2 };
            let mut src = String::new();
            let mut total = Num::zero();
            for i in 0..k {
                let d = *rng.pick(&dens[..(4 + 2 * l as usize).min(dens.len())]);
                let n = rng.nz(1, d - 1 + l);
                let neg = l >= 2 && rng.int(0, 2) == 0 && i > 0;
                src.push_str(&if i == 0 { format!("{n}/{d}") } else { format!(" {} {n}/{d}", if neg { "-" } else { "+" }) });
                let v = Num::ratio(if neg { -n } else { n }, d);
                total = total.add(&v).unwrap();
            }
            mk(src, Check::Number(total), "Answer as a reduced fraction, e.g. 7/12.", "Ответ — несократимая дробь, например 7/12.")
        }
        "det2" | "det3" => {
            let n = if topic == "det2" { 2 } else { 3 };
            let r = 2 + l;
            let mut e = Vec::new();
            for _ in 0..n * n {
                e.push(rng.int(-r, r));
            }
            let rows: Vec<String> = (0..n).map(|i| format!("[{}]", (0..n).map(|j| e[i * n + j].to_string()).collect::<Vec<_>>().join(","))).collect();
            let m = crate::eval::Mat { r: n, c: n, d: e.iter().map(|&x| Num::int(x)).collect() };
            mk(format!("det([{}])", rows.join(",")), Check::Number(m.det().unwrap()), "A whole number.", "Целое число.")
        }
        "inverse" => {
            // det = ±1 or a small number so the inverse is nice
            loop {
                let r = 1 + l;
                let (a, b, c, d) = (rng.int(-r, r), rng.int(-r, r), rng.int(-r, r), rng.int(-r, r));
                let det = a * d - b * c;
                if det == 0 || (l <= 2 && det.abs() != 1) || det.abs() > 6 {
                    continue;
                }
                let m = crate::eval::Mat { r: 2, c: 2, d: vec![Num::int(a), Num::int(b), Num::int(c), Num::int(d)] };
                let inv = m.inv().unwrap();
                return mk(format!("inv([[{a},{b}],[{c},{d}]])"), Check::Matrix(inv.d), "Answer as a matrix [[a, b], [c, d]].", "Ответ — матрица [[a, b], [c, d]].");
            }
        }
        "linear" => {
            let x = rng.int(-6 - l, 6 + l);
            let a = rng.nz(-3 - l, 3 + l);
            let b = rng.int(-9, 9);
            let c = if l >= 3 { rng.nz(-4, 4) } else { 0 };
            // a·x + b = c·x + (a − c)·x + b … choose rhs so x is the root
            let rhs = (a - c) * x + b;
            let _ = rhs;
            let src = if c != 0 { format!("{} = {}", poly_text(&[a, b]), poly_text(&[c, a * x + b - c * x])) } else { format!("{} = {}", poly_text(&[a, b]), a * x + b) };
            mk(src, Check::Roots(vec![Num::int(x)]), "Answer: the value of x.", "Ответ: значение x.")
        }
        "quadratic" => {
            let r = 2 + l;
            let (x1, x2) = (rng.int(-r, r), rng.int(-r, r));
            let a = if l >= 4 { rng.nz(-2, 3) } else { 1 };
            // a(x − x1)(x − x2)
            let (b, c) = (-a * (x1 + x2), a * x1 * x2);
            let mut roots = vec![Num::int(x1)];
            if x2 != x1 {
                roots.push(Num::int(x2));
            }
            mk(format!("{} = 0", poly_text(&[a, b, c])), Check::Roots(roots), "All roots, separated by commas: 2, -3.", "Все корни через запятую: 2, -3.")
        }
        "cubic" => {
            let r = 1 + l;
            let (x1, x2, x3) = (rng.int(-r, r), rng.int(-r, r), rng.int(-r, r));
            let (b, c, d) = (-(x1 + x2 + x3), x1 * x2 + x1 * x3 + x2 * x3, -x1 * x2 * x3);
            let mut set: Vec<i64> = vec![x1, x2, x3];
            set.sort();
            set.dedup();
            mk(format!("{} = 0", poly_text(&[1, b, c, d])), Check::Roots(set.into_iter().map(Num::int).collect()), "All roots, separated by commas (rational-root theorem helps).", "Все корни через запятую (поможет теорема о рациональных корнях).")
        }
        "system" => {
            let r = 3 + l;
            let (x, y) = (rng.int(-r, r), rng.int(-r, r));
            loop {
                let (a, b, c, d) = (rng.nz(-4, 4), rng.nz(-4, 4), rng.nz(-4, 4), rng.nz(-4, 4));
                if a * d - b * c == 0 {
                    continue;
                }
                let e1 = format!("{} = {}", poly_text_xy(a, b), a * x + b * y);
                let e2 = format!("{} = {}", poly_text_xy(c, d), c * x + d * y);
                return mk(format!("solve({e1}, {e2})"), Check::Pair(Num::int(x), Num::int(y)), "Answer: x = …, y = … (or the point (x, y)).", "Ответ: x = …, y = … (или точка (x, y)).");
            }
        }
        "derivative" => {
            let forms: Vec<String> = match l {
                1 => vec![poly_text(&[rng.nz(-5, 5), rng.int(-6, 6), rng.int(-9, 9)]), format!("{}x^{}", rng.nz(2, 7), rng.int(3, 6))],
                2 => vec![format!("{}sin(x) + {}cos(x)", rng.nz(1, 5), rng.nz(1, 5)), format!("x^{} + {}e^x", rng.int(2, 5), rng.nz(1, 4))],
                3 => vec![format!("x^{} sin(x)", rng.int(1, 3)), format!("x e^({}x)", rng.nz(2, 4)), format!("ln(x^2 + {})", rng.nz(1, 5))],
                _ => vec![format!("sin({}x^2)", rng.nz(1, 3)), format!("(x + {})/(x - {})", rng.nz(1, 5), rng.nz(1, 5)), format!("x^2 ln(x)"), format!("e^(sin x)")],
            };
            let f = rng.pick(&forms).clone();
            let e = sym::from_ast(&parse(&f, &HashSet::new()).unwrap().unwrap(), false).unwrap();
            mk(format!("diff({f})"), Check::Derivative(e), "Write f′(x) in x (any equivalent form).", "Запишите f′(x) через x (в любой равносильной форме).")
        }
        _ => {
            let forms: Vec<String> = match l {
                1 => vec![poly_text(&[rng.nz(1, 4) * 3, rng.nz(1, 4) * 2, rng.int(-5, 5)]), format!("{}x^{}", rng.nz(2, 6), rng.int(2, 5))],
                2 => vec![format!("{}cos(x) - {}sin(x)", rng.nz(1, 5), rng.nz(1, 5)), format!("e^({}x)", rng.nz(2, 5)), format!("{}/x", rng.nz(1, 5))],
                3 => vec![format!("x e^x"), format!("x cos(x)"), format!("{}x/(x^2 + 1)", rng.nz(1, 4) * 2)],
                _ => vec![format!("x^2 e^x"), format!("x sin({}x)", rng.nz(1, 3)), format!("ln(x)"), format!("x cos(x^2)")],
            };
            let f = rng.pick(&forms).clone();
            let e = sym::from_ast(&parse(&f, &HashSet::new()).unwrap().unwrap(), false).unwrap();
            mk(format!("integrate({f})"), Check::Antiderivative(e), "Write F(x) (the +C is not needed).", "Запишите F(x) (+C можно не писать).")
        }
    }
}

fn poly_text_xy(a: i64, b: i64) -> String {
    let mut s = sgn_term(a, "x", true);
    s.push_str(&sgn_term(b, "y", s.is_empty()));
    s
}

/// numbers in an answer like "x = 2; x = -3", "2, -3", "(1, 2)", "x=1 и y=2"
fn numbers_in(text: &str, env: &mut Env) -> Option<Vec<Num>> {
    let cleaned = text.replace(" и ", ",").replace(" and ", ",").replace(';', ",").replace('(', "").replace(')', "").replace('{', "").replace('}', "");
    let mut out = Vec::new();
    for part in cleaned.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let p = match p.split_once('=') {
            Some((_, r)) => r.trim(),
            None => p,
        };
        let ast = parse(p, &HashSet::new()).ok()??;
        match env.eval(&ast, &HashMap::new()) {
            Ok(Val::N(n)) => out.push(n),
            _ => return None,
        }
    }
    Some(out)
}

fn num_eq(a: &Num, b: &Num) -> bool {
    match a.sub(b) {
        Ok(Num::Q(q)) => num_traits::Zero::is_zero(&q),
        Ok(d) => d.to_f64().abs() < 1e-9 * (1.0 + b.to_f64().abs()),
        Err(_) => false,
    }
}

/// Is the user's answer right?
pub fn check(p: &Problem, text: &str) -> Result<bool, String> {
    let mut env = Env::default();
    let text = text.trim();
    if text.is_empty() {
        return Err("type an answer first".into());
    }
    Ok(match &p.check {
        Check::Number(v) => {
            let ast = parse(text.split_once('=').map(|x| x.1).unwrap_or(text), &HashSet::new()).map_err(|e| e.to_string())?.ok_or("empty")?;
            match env.eval(&ast, &HashMap::new()) {
                Ok(Val::N(n)) => num_eq(&n, v),
                _ => false,
            }
        }
        Check::Roots(rs) => {
            let mut got = numbers_in(text, &mut env).ok_or("write the roots as numbers separated by commas")?;
            got.sort_by(|a, b| a.to_f64().total_cmp(&b.to_f64()));
            got.dedup_by(|a, b| num_eq(a, b));
            let mut want = rs.clone();
            want.sort_by(|a, b| a.to_f64().total_cmp(&b.to_f64()));
            got.len() == want.len() && got.iter().zip(&want).all(|(a, b)| num_eq(a, b))
        }
        Check::Pair(x, y) => {
            let got = numbers_in(text, &mut env).ok_or("write x and y, e.g. x = 1, y = 2")?;
            got.len() == 2 && num_eq(&got[0], x) && num_eq(&got[1], y)
        }
        Check::Matrix(want) => {
            let ast = parse(text, &HashSet::new()).map_err(|e| e.to_string())?.ok_or("empty")?;
            match env.eval(&ast, &HashMap::new()) {
                Ok(Val::Mat(m)) => m.d.len() == want.len() && m.d.iter().zip(want).all(|(a, b)| num_eq(a, b)),
                _ => return Err("write a matrix like [[1, 2], [3, 4]]".into()),
            }
        }
        Check::Derivative(f) => {
            let ast = parse(text.split_once('=').map(|x| x.1).unwrap_or(text), &HashSet::new()).map_err(|e| e.to_string())?.ok_or("empty")?;
            let user = sym::from_ast(&ast, false).ok_or("write the derivative as an expression in x")?;
            let mut st = Vec::new();
            let d = crate::calc::deriv(f, "x", &mut st, 9);
            agree(&user, &d, 0.0)
        }
        Check::Antiderivative(f) => {
            let ast = parse(text.trim_end_matches("+ C").trim_end_matches("+C").split_once('=').map(|x| x.1).unwrap_or(text.trim_end_matches("+ C").trim_end_matches("+C")), &HashSet::new())
                .map_err(|e| e.to_string())?
                .ok_or("empty")?;
            let user = sym::from_ast(&ast, false).ok_or("write the antiderivative as an expression in x")?;
            let mut st = Vec::new();
            let du = crate::calc::deriv(&user, "x", &mut st, 9);
            agree(&du, f, 0.0)
        }
    })
}

/// two functions of x agree at several points (where both are defined)
fn agree(a: &E, b: &E, _shift: f64) -> bool {
    let mut n = 0;
    for &p in &[0.37, 1.21, 2.6, 0.83, 1.77, 3.1] {
        let (u, v) = (sym::eval_at(a, "x", p), sym::eval_at(b, "x", p));
        if !(u.is_finite() && v.is_finite()) {
            continue;
        }
        if (u - v).abs() > 1e-7 * (1.0 + v.abs()) {
            return false;
        }
        n += 1;
    }
    n >= 3
}

// ------------------------------------------------------------------ rating and storage

pub struct Trainer {
    pub ratings: HashMap<String, (f64, u32, u32)>, // topic → (rating, answered, correct)
    pub current: Option<Problem>,
    pub streak: u32,
    rng: Rng,
    next_pid: u64,
    path: PathBuf,
}

impl Trainer {
    pub fn load(dir: &Path) -> Trainer {
        let path = dir.join("trainer.json");
        let mut ratings = HashMap::new();
        if let Ok(t) = std::fs::read(&path) {
            if let Ok(v) = serde_json::from_slice::<Value>(&t) {
                if let Some(m) = v["ratings"].as_object() {
                    for (k, r) in m {
                        ratings.insert(k.clone(), (r["r"].as_f64().unwrap_or(1000.0), r["n"].as_u64().unwrap_or(0) as u32, r["ok"].as_u64().unwrap_or(0) as u32));
                    }
                }
            }
        }
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
        Trainer { ratings, current: None, streak: 0, rng: Rng::new(seed), next_pid: 1, path }
    }
    fn save(&self) {
        let m: serde_json::Map<String, Value> = self.ratings.iter().map(|(k, (r, n, ok))| (k.clone(), json!({"r": r, "n": n, "ok": ok}))).collect();
        if let Some(d) = self.path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&json!({"version": 1, "ratings": m})).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, &self.path);
        }
    }
    fn rating(&self, topic: &str) -> f64 {
        self.ratings.get(topic).map(|r| r.0).unwrap_or(1000.0)
    }
    /// level 1…5 so the chance of success is about 75 % (each level ≈ 120 rating points)
    fn level_for(&self, topic: &str) -> i32 {
        (((self.rating(topic) - 1000.0 + 190.0) / 120.0).floor() as i32 + 1).clamp(1, 5)
    }
    pub fn next(&mut self, topic: Option<&str>) -> &Problem {
        let topic = match topic.filter(|t| TOPICS.iter().any(|x| x.0 == *t)) {
            Some(t) => t.to_string(),
            None => self.rng.pick(TOPICS).0.to_string(),
        };
        let level = self.level_for(&topic);
        let pid = self.next_pid;
        self.next_pid += 1;
        self.current = Some(generate(&topic, level, &mut self.rng, pid));
        self.current.as_ref().unwrap()
    }
    /// record the result; returns (correct, new rating)
    pub fn answer(&mut self, pid: u64, text: &str) -> Result<(bool, f64), String> {
        let p = self.current.as_ref().filter(|p| p.pid == pid).ok_or("that problem is no longer open")?;
        let ok = check(p, text)?;
        let topic = p.topic.clone();
        let diff = 1000.0 + (p.level as f64 - 1.0) * 120.0 - 190.0 + 60.0;
        let e = self.ratings.entry(topic).or_insert((1000.0, 0, 0));
        let expected = 1.0 / (1.0 + 10f64.powf((diff - e.0) / 400.0));
        e.0 += 32.0 * ((ok as i32 as f64) - expected);
        e.1 += 1;
        e.2 += ok as u32;
        let r = e.0;
        self.streak = if ok { self.streak + 1 } else { 0 };
        self.save();
        Ok((ok, r))
    }
    pub fn stats(&self) -> Value {
        json!(TOPICS.iter().map(|(k, en, ru)| {
            let (r, n, ok) = self.ratings.get(*k).cloned().unwrap_or((1000.0, 0, 0));
            json!({"id": k, "en": en, "ru": ru, "rating": r.round(), "level": self.level_for(k), "answered": n, "correct": ok})
        }).collect::<Vec<_>>())
    }
}

/// The problem as shown: its math (the input as written) and a short task line.
pub fn problem_json(p: &Problem) -> Value {
    let ast: Ast = parse(&p.source, &HashSet::new()).ok().flatten().unwrap_or(Ast::Num("0".into()));
    let (en, ru) = match p.topic.as_str() {
        "fractions" => ("Compute:", "Вычислите:"),
        "det2" | "det3" => ("Compute the determinant:", "Вычислите определитель:"),
        "inverse" => ("Find the inverse matrix:", "Найдите обратную матрицу:"),
        "linear" | "quadratic" | "cubic" => ("Solve the equation:", "Решите уравнение:"),
        "system" => ("Solve the system:", "Решите систему:"),
        "derivative" => ("Find the derivative:", "Найдите производную:"),
        _ => ("Find the antiderivative:", "Найдите первообразную:"),
    };
    json!({"pid": p.pid, "topic": p.topic, "level": p.level, "source": p.source, "task": {"en": en, "ru": ru},
           "math": crate::mtree::ast(&ast), "hint": {"en": p.hint_en, "ru": p.hint_ru}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_problems_check_their_own_answers() {
        let mut rng = Rng::new(12345);
        for (topic, _, _) in TOPICS {
            for level in 1..=5 {
                let p = generate(topic, level, &mut rng, 1);
                // the problem must parse and be solvable with steps
                let ast = parse(&p.source, &HashSet::new()).unwrap().unwrap();
                let mut env = Env::default();
                let sol = crate::steps::solve(&ast, &mut env);
                assert!(sol.is_ok(), "{topic} L{level}: {} → {:?}", p.source, sol.err());
                // and the stored answer must be accepted when typed back
                let typed = match &p.check {
                    Check::Number(n) => n.plain(20),
                    Check::Roots(r) => r.iter().map(|x| x.plain(20)).collect::<Vec<_>>().join(", "),
                    Check::Pair(x, y) => format!("x = {}, y = {}", x.plain(20), y.plain(20)),
                    Check::Matrix(m) => format!("[[{}, {}], [{}, {}]]", m[0].plain(20), m[1].plain(20), m[2].plain(20), m[3].plain(20)),
                    Check::Derivative(f) => {
                        let mut st = Vec::new();
                        sym::plain(&crate::calc::deriv(f, "x", &mut st, 9))
                    }
                    Check::Antiderivative(f) => {
                        let mut st = Vec::new();
                        crate::calc::integ(f, "x", &mut st, 0).map(|e| sym::plain(&e)).expect("integrable")
                    }
                };
                assert!(check(&p, &typed).unwrap_or(false), "{topic} L{level}: {} — typed {typed:?} was rejected", p.source);
            }
        }
        // wrong answers are rejected
        let p = Problem { pid: 1, topic: "quadratic".into(), level: 1, source: "x^2 - 5x + 6 = 0".into(), check: Check::Roots(vec![Num::int(2), Num::int(3)]), hint_en: String::new(), hint_ru: String::new() };
        assert!(check(&p, "x = 3; x = 2").unwrap());
        assert!(!check(&p, "2, 4").unwrap());
    }
}
