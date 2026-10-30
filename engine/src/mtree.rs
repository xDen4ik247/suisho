//! Math layout trees for the window (MathView.qml lays them out and draws them):
//!   {"k":"t","s":"x","it":1}        text (it: italic variable, op: binary operator spacing)
//!   {"k":"row","c":[…]}              a sequence
//!   {"k":"frac","n":…,"d":…}         fraction
//!   {"k":"sup","b":…,"e":…}          power / superscript;  {"k":"sub",…} subscript
//!   {"k":"sqrt","a":…,"i":"3"}       root (i: index)
//!   {"k":"par","a":…,"o":"(","c":")"} stretchy brackets ( ) [ ] | |
//!   {"k":"mat","r":[[…]],"aug":n,"hr":[rows],"o":"("} matrix (aug: bar before column n; hr: highlighted rows)
//!   {"k":"hl","a":…,"h":"chg|piv|good|bad"} highlight

use crate::eval::{Mat, Val};
use crate::num::Num;
use crate::parse::{Ast, Rel};
use crate::sym::E;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed};
use serde_json::{json, Value};

pub fn t(s: &str) -> Value {
    json!({"k": "t", "s": s})
}
pub fn var(s: &str) -> Value {
    json!({"k": "t", "s": s, "it": 1})
}
pub fn op(s: &str) -> Value {
    json!({"k": "t", "s": s, "op": 1})
}
pub fn row(c: Vec<Value>) -> Value {
    if c.len() == 1 {
        return c.into_iter().next().unwrap();
    }
    json!({"k": "row", "c": c})
}
pub fn frac(n: Value, d: Value) -> Value {
    json!({"k": "frac", "n": n, "d": d})
}
pub fn sup(b: Value, e: Value) -> Value {
    json!({"k": "sup", "b": b, "e": e})
}
pub fn sub(b: Value, e: Value) -> Value {
    json!({"k": "sub", "b": b, "e": e})
}
pub fn sqrt(a: Value, idx: Option<&str>) -> Value {
    match idx {
        Some(i) => json!({"k": "sqrt", "a": a, "i": i}),
        None => json!({"k": "sqrt", "a": a}),
    }
}
pub fn par(a: Value) -> Value {
    json!({"k": "par", "a": a, "o": "(", "c": ")"})
}
pub fn bars(a: Value) -> Value {
    json!({"k": "par", "a": a, "o": "|", "c": "|"})
}
#[allow(dead_code)]
pub fn hl(a: Value, h: &str) -> Value {
    json!({"k": "hl", "a": a, "h": h})
}
#[allow(dead_code)]
pub fn text_en_ru(en: &str, ru: &str, lang_ru: bool) -> Value {
    t(if lang_ru { ru } else { en })
}

/// R₂ (row names in steps)
pub fn rname(i: usize) -> Value {
    sub(var("R"), t(&(i + 1).to_string()))
}

pub fn int(n: &BigInt) -> Value {
    if n.is_negative() {
        row(vec![t("−"), t(&(-n).to_string())])
    } else {
        t(&n.to_string())
    }
}

/// A rational as a (signed) fraction.
pub fn rat(r: &BigRational) -> Value {
    if r.is_integer() {
        return int(&r.to_integer());
    }
    let f = frac(t(&r.numer().abs().to_string()), t(&r.denom().to_string()));
    if r.is_negative() {
        row(vec![t("−"), f])
    } else {
        f
    }
}

pub fn num(n: &Num) -> Value {
    match n {
        Num::Q(r) => rat(r),
        Num::R(_) => {
            let s = n.plain(10);
            if let Some(rest) = s.strip_prefix('-') {
                row(vec![t("−"), t(rest)])
            } else {
                t(&s)
            }
        }
    }
}

/// Wrap negative numbers in parentheses (for "2·(−3)").
pub fn num_p(n: &Num) -> Value {
    if n.sign() < 0 {
        par(num(n))
    } else {
        num(n)
    }
}

pub fn mat(m: &Mat) -> Value {
    let r: Vec<Vec<Value>> = (0..m.r).map(|i| (0..m.c).map(|j| num(m.at(i, j))).collect()).collect();
    json!({"k": "mat", "r": r, "o": "("})
}

pub fn val(v: &Val) -> Value {
    match v {
        Val::N(n) => num(n),
        Val::Mat(m) => mat(m),
        Val::Bool(b) => t(if *b { "true" } else { "false" }),
        Val::List(l) => {
            let mut c = vec![t("[")];
            for (i, x) in l.iter().enumerate() {
                if i > 0 {
                    c.push(t(", "));
                }
                c.push(val(x));
            }
            c.push(t("]"));
            row(c)
        }
        Val::Tuple(l) => {
            let mut c = vec![];
            for (i, x) in l.iter().enumerate() {
                if i > 0 {
                    c.push(t(", "));
                }
                c.push(val(x));
            }
            par(row(c))
        }
    }
}

// ------------------------------------------------------------------ symbolic expressions

pub fn sym(e: &E) -> Value {
    match e {
        E::Q(r) => rat(r),
        E::Sym(s) => sym_name(s),
        E::Pi => t("π"),
        E::Euler => var("e"),
        E::Add(v) => {
            let mut c = Vec::new();
            for (i, term) in v.iter().enumerate() {
                let (neg, body) = split_neg(term);
                if i == 0 {
                    if neg {
                        c.push(t("−"));
                    }
                } else {
                    c.push(op(if neg { "−" } else { "+" }));
                }
                c.push(body);
            }
            row(c)
        }
        E::Mul(_) => {
            let (neg, body) = split_neg(e);
            if neg {
                row(vec![t("−"), body])
            } else {
                body
            }
        }
        E::Pow(b, x) => pow_tree(b, x),
        E::Fn(f, a) if a.is_empty() => t(f), // ∞, +∞, −∞ from limits
        E::Fn(f, a) => {
            let inner: Vec<Value> = a.iter().map(sym).collect();
            if f == "abs" && inner.len() == 1 {
                return bars(inner.into_iter().next().unwrap());
            }
            row(vec![t(f), par(row(inner))])
        }
    }
}

fn sym_name(s: &str) -> Value {
    let greek = match s {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "theta" => "θ",
        "lambda" => "λ",
        "mu" => "μ",
        "phi" => "φ",
        "omega" => "ω",
        "sigma" => "σ",
        "rho" => "ρ",
        "tau" => "τ",
        _ => "",
    };
    if !greek.is_empty() {
        return var(greek);
    }
    if let Some((b, i)) = s.split_once('_') {
        return sub(var(b), t(i));
    }
    var(s)
}

fn pow_tree(b: &E, x: &E) -> Value {
    if let E::Q(r) = x {
        if r.numer().is_one() && r.denom() > &BigInt::one() {
            let idx = r.denom().to_string();
            return sqrt(sym(b), if idx == "2" { None } else { Some(&idx) });
        }
        if r.is_negative() {
            return frac(t("1"), pow_tree(b, &E::Q(-r.clone())));
        }
        if !r.is_integer() {
            // a^(p/q) = (q-th root of a)^p
            let root = pow_tree(b, &E::Q(BigRational::new(BigInt::one(), r.denom().clone())));
            return sup(root, t(&r.numer().to_string()));
        }
    }
    let base = match b {
        E::Add(_) | E::Mul(_) | E::Pow(..) => par(sym(b)),
        E::Q(r) if r.is_negative() || !r.is_integer() => par(sym(b)),
        _ => sym(b),
    };
    sup(base, sym(x))
}

/// A product as "coefficient · factors" or a fraction when there are denominators; reports the sign.
fn split_neg(e: &E) -> (bool, Value) {
    match e {
        E::Q(r) if r.is_negative() => (true, rat(&-r.clone())),
        E::Mul(v) => {
            let mut c = BigRational::one();
            let mut num_f: Vec<&E> = Vec::new();
            let mut den_f: Vec<E> = Vec::new();
            for f in v {
                match f {
                    E::Q(r) => c *= r,
                    E::Pow(b, x) if matches!(&**x, E::Q(r) if r.is_negative() && r.is_integer()) => {
                        let E::Q(r) = &**x else { unreachable!() };
                        den_f.push(if r == &BigRational::from_integer(BigInt::from(-1)) {
                            (**b).clone()
                        } else {
                            E::Pow(b.clone(), Box::new(E::Q(-r.clone())))
                        });
                    }
                    o => num_f.push(o),
                }
            }
            let neg = c.is_negative();
            let c = c.abs();
            let mut numer: Vec<Value> = Vec::new();
            if !c.numer().is_one() || num_f.is_empty() {
                numer.push(t(&c.numer().to_string()));
            }
            for f in &num_f {
                numer.push(match f {
                    E::Add(_) => par(sym(f)),
                    _ => sym(f),
                });
            }
            let mut denom: Vec<Value> = Vec::new();
            if !c.denom().is_one() {
                denom.push(t(&c.denom().to_string()));
            }
            for f in &den_f {
                denom.push(match f {
                    E::Add(_) => par(sym(f)),
                    _ => sym(f),
                });
            }
            let n = row(numer);
            (neg, if denom.is_empty() { n } else { frac(n, row(denom)) })
        }
        o => (false, sym(o)),
    }
}

// ------------------------------------------------------------------ input as written (for the steps' header)

pub fn ast(a: &Ast) -> Value {
    match a {
        Ast::Num(s) => t(s),
        Ast::Var(v) => match v.as_str() {
            "pi" => t("π"),
            "∞" => t("∞"),
            v => sym_name(v),
        },
        Ast::Neg(x) => row(vec![t("−"), ast_p(x, 2)]),
        Ast::Add(x, y) => row(vec![ast(x), op("+"), ast_p(y, 1)]),
        Ast::Sub(x, y) => row(vec![ast(x), op("−"), ast_p(y, 1)]),
        Ast::Mul(x, y, implicit) => {
            if *implicit {
                row(vec![ast_p(x, 2), ast_p(y, 2)])
            } else {
                row(vec![ast_p(x, 2), op("·"), ast_p(y, 2)])
            }
        }
        Ast::Div(x, y) => frac(ast(x), ast(y)),
        Ast::Pow(x, y) => sup(ast_p(x, 3), ast(y)),
        Ast::Fact(x) => row(vec![ast_p(x, 3), t("!")]),
        Ast::Deg(x) => row(vec![ast_p(x, 3), t("°")]),
        Ast::Abs(x) => bars(ast(x)),
        Ast::Call(f, args) => match (f.as_str(), args.len()) {
            ("sqrt", 1) => sqrt(ast(&args[0]), None),
            ("cbrt", 1) => sqrt(ast(&args[0]), Some("3")),
            ("root", 2) => {
                let i = if let Ast::Num(n) = &args[1] { n.clone() } else { "n".into() };
                sqrt(ast(&args[0]), Some(&i))
            }
            ("abs", 1) => bars(ast(&args[0])),
            ("integrate" | "int", 1) => row(vec![t("∫"), ast(&args[0]), t(" d"), var("x")]),
            ("integrate" | "int", 3) => row(vec![json!({"k": "sub", "b": sup(t("∫"), ast(&args[2])), "e": ast(&args[1])}), ast(&args[0]), t(" d"), var("x")]),
            ("diff" | "derivative", 1) => sup(par(ast(&args[0])), t("′")),
            ("lim" | "limit", 3) => row(vec![sub(t("lim"), row(vec![ast(&args[1]), t("→"), ast(&args[2])])), t(" "), ast(&args[0])]),
            ("lim" | "limit", 2) => row(vec![sub(t("lim"), row(vec![var("x"), t("→"), ast(&args[1])])), t(" "), ast(&args[0])]),
            ("det", 1) if matches!(args[0], Ast::Matrix(_)) => {
                let mut m = ast(&args[0]);
                m["o"] = json!("|");
                m
            }
            _ => {
                let mut c = Vec::new();
                for (i, x) in args.iter().enumerate() {
                    if i > 0 {
                        c.push(t(", "));
                    }
                    c.push(ast(x));
                }
                row(vec![t(f), par(row(c))])
            }
        },
        Ast::List(items) => {
            let mut c = vec![t("[")];
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    c.push(t(", "));
                }
                c.push(ast(x));
            }
            c.push(t("]"));
            row(c)
        }
        Ast::Matrix(rows) => json!({"k": "mat", "o": "(", "r": rows.iter().map(|r| r.iter().map(ast).collect::<Vec<_>>()).collect::<Vec<_>>()}),
        Ast::Tuple(items) => {
            let mut c = vec![];
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    c.push(t(", "));
                }
                c.push(ast(x));
            }
            par(row(c))
        }
        Ast::Rel(first, chain) => {
            let mut c = vec![ast(first)];
            for (r, e) in chain {
                c.push(op(rel_sym(*r)));
                c.push(ast(e));
            }
            row(c)
        }
        Ast::Piecewise(..) | Ast::Restrict(..) => t("…"),
    }
}

fn rel_sym(r: Rel) -> &'static str {
    r.sym()
}

/// Parenthesise by precedence: 1 = right side of ±, 2 = factor, 3 = power base.
fn ast_p(a: &Ast, level: u8) -> Value {
    let need = match a {
        Ast::Add(..) | Ast::Sub(..) => level >= 1,
        Ast::Neg(_) => level >= 2,
        Ast::Mul(..) | Ast::Div(..) => level >= 3,
        Ast::Pow(..) => level >= 3,
        Ast::Num(s) => level >= 3 && s.contains('.'),
        _ => false,
    };
    if need {
        par(ast(a))
    } else {
        ast(a)
    }
}

/// The tree as plain text (for tests and the row's text value).
pub fn flat(v: &Value) -> String {
    match v["k"].as_str() {
        Some("t") => v["s"].as_str().unwrap_or("").to_string(),
        Some("row") => v["c"].as_array().map(|c| c.iter().map(flat).collect()).unwrap_or_default(),
        Some("frac") => format!("{}/{}", flat(&v["n"]), flat(&v["d"])),
        Some("sqrt") => match v["i"].as_str() {
            Some(i) => format!("root({}, {i})", flat(&v["a"])),
            None => format!("√{}", flat(&v["a"])),
        },
        Some("sup") => format!("{}^{}", flat(&v["b"]), flat(&v["e"])),
        Some("sub") => format!("{}_{}", flat(&v["b"]), flat(&v["e"])),
        Some("par") => format!("{}{}{}", v["o"].as_str().unwrap_or("("), flat(&v["a"]), v["c"].as_str().unwrap_or(")")),
        Some("hl") => flat(&v["a"]),
        Some("mat") => "[matrix]".into(),
        _ => String::new(),
    }
}

/// The tree as LaTeX.
pub fn latex(v: &Value) -> String {
    let greek = |s: &str| -> String {
        let map = [("π", "\\pi "), ("λ", "\\lambda "), ("θ", "\\theta "), ("α", "\\alpha "), ("β", "\\beta "), ("φ", "\\varphi "), ("Δ", "\\Delta "),
                   ("δ", "\\delta "), ("∞", "\\infty "), ("−", "-"), ("·", "\\cdot "), ("≤", "\\le "), ("≥", "\\ge "), ("≠", "\\ne "), ("→", "\\to "),
                   ("←", "\\leftarrow "), ("↔", "\\leftrightarrow "), ("⇒", "\\Rightarrow "), ("±", "\\pm "), ("∈", "\\in "), ("∪", "\\cup "),
                   ("ℝ", "\\mathbb{R}"), ("∅", "\\varnothing "), ("∫", "\\int "), ("′", "'"), ("×", "\\times "), ("°", "^\\circ "), ("∄", "\\nexists ")];
        let mut out = s.to_string();
        for (a, b) in map {
            out = out.replace(a, b);
        }
        out
    };
    match v["k"].as_str() {
        Some("t") => {
            let s = v["s"].as_str().unwrap_or("");
            if v["it"].as_i64() == Some(1) || s.chars().all(|c| c.is_ascii_digit() || "+-=()[],.;: ".contains(c) || "−·≤≥≠→←⇒±∈∪ℝ∅∫′×°∞πλθαβφΔδ∄".contains(c)) {
                greek(s)
            } else if s.trim().is_empty() {
                "\\;".into()
            } else {
                format!("\\text{{{}}}", s.replace('{', "\\{").replace('}', "\\}"))
            }
        }
        Some("row") => v["c"].as_array().map(|c| c.iter().map(latex).collect::<Vec<_>>().join("")).unwrap_or_default(),
        Some("frac") => format!("\\frac{{{}}}{{{}}}", latex(&v["n"]), latex(&v["d"])),
        Some("sup") => format!("{{{}}}^{{{}}}", latex(&v["b"]), latex(&v["e"])),
        Some("sub") => format!("{{{}}}_{{{}}}", latex(&v["b"]), latex(&v["e"])),
        Some("sqrt") => match v["i"].as_str() {
            Some(i) => format!("\\sqrt[{i}]{{{}}}", latex(&v["a"])),
            None => format!("\\sqrt{{{}}}", latex(&v["a"])),
        },
        Some("par") => {
            let (o, c) = (v["o"].as_str().unwrap_or("("), v["c"].as_str().unwrap_or(")"));
            let br = |s: &str| match s { "|" => "|".to_string(), "[" => "[".into(), "]" => "]".into(), "{" => "\\{".into(), "}" => "\\}".into(), x => x.to_string() };
            format!("\\left{}{}\\right{}", br(o), latex(&v["a"]), br(c))
        }
        Some("mat") => {
            let env = match v["o"].as_str() { Some("|") => "vmatrix", Some("[") => "bmatrix", _ => "pmatrix" };
            let aug = v["aug"].as_u64();
            let rows: Vec<String> = v["r"].as_array().map(|r| r.iter().map(|row| {
                row.as_array().map(|cells| cells.iter().enumerate().map(|(j, c)| {
                    let s = latex(c);
                    if aug == Some(j as u64) { format!("\\vline\\; {s}") } else { s }
                }).collect::<Vec<_>>().join(" & ")).unwrap_or_default()
            }).collect()).unwrap_or_default();
            format!("\\begin{{{env}}}{}\\end{{{env}}}", rows.join(" \\\\ "))
        }
        Some("hl") => latex(&v["a"]),
        _ => String::new(),
    }
}

/// A whole solution (as sent to the window) as Markdown with $$ math, or as a LaTeX document body.
pub fn export_solution(sol: &Value, fmt: &str, ru: bool) -> String {
    let lang = if ru { "ru" } else { "en" };
    let mut out = String::new();
    let md = fmt != "latex";
    let title = sol[lang].as_str().unwrap_or("");
    if md {
        out.push_str(&format!("### {title}\n\n$${}$$\n\n", latex(&sol["problem"])));
    } else {
        out.push_str(&format!("\\subsection*{{{title}}}\n\\[ {} \\]\n\\begin{{enumerate}}\n", latex(&sol["problem"])));
    }
    fn steps(list: &Value, depth: usize, md: bool, lang: &str, out: &mut String) {
        for (i, s) in list.as_array().into_iter().flatten().enumerate() {
            let words = s[lang].as_str().unwrap_or("");
            let head = if s["head"].is_null() { String::new() } else { format!(" ${}$", latex(&s["head"])) };
            if md {
                out.push_str(&format!("{}{}. {}{}\n", "   ".repeat(depth), i + 1, words, head));
                for m in s["math"].as_array().into_iter().flatten() {
                    out.push_str(&format!("{}   $${}$$\n", "   ".repeat(depth), latex(m)));
                }
            } else {
                out.push_str(&format!("\\item {}{}\n", words.replace('%', "\\%"), head));
                for m in s["math"].as_array().into_iter().flatten() {
                    out.push_str(&format!("\\[ {} \\]\n", latex(m)));
                }
            }
            if s["sub"].as_array().is_some_and(|a| !a.is_empty()) {
                if !md {
                    out.push_str("\\begin{enumerate}\n");
                }
                steps(&s["sub"], depth + 1, md, lang, out);
                if !md {
                    out.push_str("\\end{enumerate}\n");
                }
            }
        }
    }
    steps(&sol["steps"], 0, md, lang, &mut out);
    let ans = if ru { "Ответ" } else { "Answer" };
    if md {
        out.push_str(&format!("\n**{ans}:** ${}$\n", latex(&sol["answer"])));
    } else {
        out.push_str(&format!("\\end{{enumerate}}\n\\textbf{{{ans}:}} \\( {} \\)\n", latex(&sol["answer"])));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latex_out() {
        let v = row(vec![frac(t("1"), t("2")), op("+"), sqrt(t("2"), None), op("·"), sup(var("x"), t("2"))]);
        assert_eq!(latex(&v), "\\frac{1}{2}+\\sqrt{2}\\cdot {x}^{2}");
    }
}
