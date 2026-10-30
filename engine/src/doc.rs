//! The workspace: an ordered list of rows (like Desmos), each one an expression, a definition
//! (a = 3, f(x) = x²), a graphable relation (y = x², x² + y² < 4) or empty. Definitions may come in any
//! order; the whole document is recomputed on every change (cheap at this size). Autosaved as JSON.

use crate::eval::{approx, plain, Env, EvalErr, Val};
use crate::parse::{definition_head, parse, Ast, Rel};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Letters that make a row a graph instead of an error when they are not defined.
pub const GRAPH_VARS: &[&str] = &["x", "y", "r", "theta", "t"];

#[derive(Clone)]
pub struct Row {
    pub id: u64,
    pub text: String,
    pub color: u32,
    pub hidden: bool,
}

pub struct Doc {
    pub id: String,
    pub name: String,
    pub rows: Vec<Row>,
    next_id: u64,
    next_color: u32,
    undo: Vec<Vec<Row>>,
    redo: Vec<Vec<Row>>,
    pub dirty: bool,
}

#[derive(Clone, Copy)]
pub struct Opts {
    pub degrees: bool,
    pub digits: usize,
}

impl Doc {
    pub fn new(id: String, name: String) -> Doc {
        let mut d = Doc { id, name, rows: vec![], next_id: 1, next_color: 0, undo: vec![], redo: vec![], dirty: true };
        d.push_row(String::new());
        d
    }

    fn push_row(&mut self, text: String) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let color = self.next_color;
        self.next_color = (self.next_color + 1) % 8;
        self.rows.push(Row { id, text, color, hidden: false });
        id
    }

    fn snapshot(&mut self) {
        self.undo.push(self.rows.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn set_text(&mut self, id: Option<u64>, text: &str) -> u64 {
        self.dirty = true;
        match id.and_then(|id| self.rows.iter_mut().find(|r| r.id == id)) {
            Some(r) => {
                r.text = text.to_string();
                r.id
            }
            None => self.push_row(text.to_string()),
        }
    }

    pub fn insert_after(&mut self, after: Option<u64>, text: &str) -> u64 {
        self.snapshot();
        self.dirty = true;
        let id = self.push_row(text.to_string());
        let row = self.rows.pop().unwrap();
        let at = after.and_then(|a| self.rows.iter().position(|r| r.id == a)).map(|i| i + 1).unwrap_or(self.rows.len());
        self.rows.insert(at, row);
        id
    }

    pub fn delete(&mut self, id: u64) {
        self.snapshot();
        self.dirty = true;
        self.rows.retain(|r| r.id != id);
        if self.rows.is_empty() {
            self.push_row(String::new());
        }
    }

    pub fn move_row(&mut self, id: u64, to: usize) {
        if let Some(i) = self.rows.iter().position(|r| r.id == id) {
            self.snapshot();
            self.dirty = true;
            let r = self.rows.remove(i);
            self.rows.insert(to.min(self.rows.len()), r);
        }
    }

    pub fn set_hidden(&mut self, id: u64, hidden: bool) {
        if let Some(r) = self.rows.iter_mut().find(|r| r.id == id) {
            r.hidden = hidden;
            self.dirty = true;
        }
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.rows, prev));
                self.dirty = true;
                true
            }
            None => false,
        }
    }
    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.rows, next));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// The environment (all definitions) and every row parsed.
    #[allow(clippy::type_complexity)]
    fn prepare(&self, o: Opts) -> (Env, Vec<Result<Option<Ast>, String>>, HashMap<usize, EvalErr>) {
        let mut env = Env { degrees: o.degrees, ..Default::default() };
        // pass 1: user function names (so f(x) parses as a call) and parse everything
        let mut funcs = HashSet::new();
        for r in &self.rows {
            if let Some((n, params)) = definition_head(&r.text) {
                if !params.is_empty() {
                    funcs.insert(n);
                }
            }
        }
        let parsed: Vec<Result<Option<Ast>, String>> = self.rows.iter().map(|r| parse(&r.text, &funcs)).collect();
        // pass 2: function definitions, then variable definitions until nothing new resolves (any order)
        let mut var_defs: Vec<(usize, String, Ast)> = Vec::new();
        for (i, p) in parsed.iter().enumerate() {
            let Ok(Some(ast)) = p else { continue };
            if let Some((name, params, body)) = split_def(ast) {
                if params.is_empty() {
                    if !GRAPH_VARS.contains(&name.as_str()) {
                        var_defs.push((i, name, body));
                    }
                } else {
                    env.funcs.insert(name, (params, body));
                }
            }
        }
        let mut def_err: HashMap<usize, EvalErr> = HashMap::new();
        let mut pending = var_defs;
        loop {
            let mut progress = false;
            let mut rest = Vec::new();
            for (i, name, body) in pending {
                match env.eval(&body, &HashMap::new()) {
                    Ok(v) => {
                        env.vars.insert(name, v);
                        def_err.remove(&i);
                        progress = true;
                    }
                    Err(e) => {
                        def_err.insert(i, e);
                        rest.push((i, name, body));
                    }
                }
            }
            pending = rest;
            if !progress || pending.is_empty() {
                break;
            }
        }
        (env, parsed, def_err)
    }

    /// Graph geometry of every graph row for a view.
    pub fn plot(&self, o: Opts, v: crate::plot::View) -> Value {
        let (mut env, parsed, _) = self.prepare(o);
        let rows: Vec<(u64, u32, Ast, bool)> = self
            .rows
            .iter()
            .zip(parsed)
            .filter_map(|(r, p)| match p {
                Ok(Some(a)) if !is_definition(&a) => Some((r.id, r.color, a, r.hidden)),
                _ => None,
            })
            .collect();
        crate::plot::plot(&rows, &mut env, o.degrees, v)
    }

    /// The step-by-step solution of row `id`.
    pub fn solution(&self, id: u64, o: Opts) -> Result<Value, String> {
        let (mut env, parsed, _) = self.prepare(o);
        let i = self.rows.iter().position(|r| r.id == id).ok_or("no such row")?;
        let ast = parsed[i].clone()?.ok_or("the row is empty")?;
        Ok(crate::steps::solve(&ast, &mut env)?.json())
    }

    /// Evaluate every row; returns the rows as JSON for the window.
    pub fn compute(&self, o: Opts) -> Value {
        let (mut env, parsed, def_err) = self.prepare(o);
        // pass 3: one result per row
        let rows: Vec<Value> = self
            .rows
            .iter()
            .zip(&parsed)
            .enumerate()
            .map(|(i, (r, p))| {
                let mut out = json!({"id": r.id, "text": r.text, "color": r.color, "hidden": r.hidden});
                let ast = match p {
                    Err(e) => {
                        out["kind"] = json!("error");
                        out["error"] = json!(e);
                        return out;
                    }
                    Ok(None) => {
                        out["kind"] = json!("empty");
                        return out;
                    }
                    Ok(Some(a)) => a,
                };
                if crate::steps::available(ast) {
                    out["steps"] = json!(true);
                }
                if let Some((name, params, _)) = split_def(ast) {
                    if !params.is_empty() {
                        out["kind"] = json!("func");
                        out["name"] = json!(name);
                        out["params"] = json!(params);
                        return out;
                    }
                    if !GRAPH_VARS.contains(&name.as_str()) {
                        out["kind"] = json!("def");
                        out["name"] = json!(name);
                        match (env.vars.get(&name), def_err.get(&i)) {
                            (Some(v), _) => fill_value(&mut out, v, o.digits),
                            (None, Some(e)) => fill_err(&mut out, e),
                            _ => {}
                        }
                        return out;
                    }
                }
                // exact symbolic form when the row is pure arithmetic with π, e, roots (√8 = 2√2, π/2 + π/3 = 5π/6)
                if let Some(e) = crate::sym::from_ast(ast, o.degrees).filter(|e| !e.has_sym()) {
                    let x = e.to_f64();
                    if x.is_finite() {
                        out["kind"] = json!("expr");
                        out["value"] = json!(crate::sym::plain(&e));
                        out["m"] = crate::mtree::sym(&e);
                        if !matches!(e.is_q(), Some(r) if r.is_integer()) {
                            out["approx"] = json!(crate::num::fmt_float(x, o.digits));
                        }
                        out["vtype"] = json!("number");
                        out["exact"] = json!(true);
                        return out;
                    }
                }
                match env.eval(ast, &HashMap::new()) {
                    Ok(Val::Bool(b)) if is_relation(ast) => {
                        out["kind"] = json!("rel");
                        out["value"] = json!(if b { "true" } else { "false" });
                    }
                    Ok(v) => {
                        out["kind"] = json!("expr");
                        fill_value(&mut out, &v, o.digits);
                    }
                    Err(EvalErr::Free(n)) if GRAPH_VARS.contains(&n.as_str()) => {
                        out["kind"] = json!("graph");
                    }
                    Err(e) => fill_err(&mut out, &e),
                }
                out
            })
            .collect();
        json!(rows)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "name": self.name, "version": 1,
            "rows": self.rows.iter().map(|r| json!({"id": r.id, "text": r.text, "color": r.color, "hidden": r.hidden})).collect::<Vec<_>>(),
        })
    }

    pub fn from_json(v: &Value) -> Option<Doc> {
        let id = v["id"].as_str()?.to_string();
        let name = v["name"].as_str().unwrap_or("Untitled").to_string();
        let mut d = Doc { id, name, rows: vec![], next_id: 1, next_color: 0, undo: vec![], redo: vec![], dirty: false };
        for r in v["rows"].as_array()? {
            let id = r["id"].as_u64().unwrap_or(d.next_id);
            let color = r["color"].as_u64().unwrap_or(0) as u32;
            d.next_id = d.next_id.max(id + 1);
            d.next_color = (color + 1) % 8;
            d.rows.push(Row { id, text: r["text"].as_str().unwrap_or("").into(), color, hidden: r["hidden"].as_bool().unwrap_or(false) });
        }
        if d.rows.is_empty() {
            d.push_row(String::new());
        }
        Some(d)
    }

    pub fn save(&mut self, dir: &Path) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        std::fs::create_dir_all(dir)?;
        let p = dir.join(format!("{}.json", self.id));
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.to_json())?)?;
        std::fs::rename(tmp, p)?;
        self.dirty = false;
        Ok(())
    }
}

fn fill_value(out: &mut Value, v: &Val, digits: usize) {
    out["value"] = json!(plain(v, digits));
    out["m"] = crate::mtree::val(v);
    if let Some(a) = approx(v, digits) {
        out["approx"] = json!(a);
    }
    out["vtype"] = json!(match v {
        Val::N(_) => "number",
        Val::List(_) => "list",
        Val::Mat(m) => {
            out["shape"] = json!([m.r, m.c]);
            out["cells"] = json!((0..m.r).map(|i| (0..m.c).map(|j| m.at(i, j).plain(digits)).collect::<Vec<_>>()).collect::<Vec<_>>());
            "matrix"
        }
        Val::Tuple(_) => "point",
        Val::Bool(_) => "bool",
    });
}

fn fill_err(out: &mut Value, e: &EvalErr) {
    match e {
        EvalErr::Free(n) => {
            out["kind"] = json!("error");
            out["error"] = json!(format!("{n} is not defined"));
            out["free"] = json!([n]);
        }
        EvalErr::Msg(m) => {
            if out["kind"].is_null() {
                out["kind"] = json!("error");
            }
            out["error"] = json!(m);
        }
    }
}

/// `a = 3` / `f(x) = …` (not graphs like y = …, x = …).
fn is_definition(a: &Ast) -> bool {
    match split_def(a) {
        Some((name, params, _)) => !params.is_empty() || !GRAPH_VARS.contains(&name.as_str()),
        None => false,
    }
}

fn is_relation(a: &Ast) -> bool {
    match a {
        Ast::Rel(..) => true,
        Ast::Restrict(e, _) => is_relation(e),
        _ => false,
    }
}

/// `name = body` / `f(x, y) = body` → (name, params, body).
fn split_def(a: &Ast) -> Option<(String, Vec<String>, Ast)> {
    let Ast::Rel(lhs, chain) = a else { return None };
    if chain.len() != 1 || chain[0].0 != Rel::Eq {
        return None;
    }
    let body = chain[0].1.clone();
    match &**lhs {
        Ast::Var(n) => Some((n.clone(), vec![], body)),
        Ast::Call(n, args) => {
            let params: Option<Vec<String>> = args.iter().map(|x| if let Ast::Var(p) = x { Some(p.clone()) } else { None }).collect();
            params.map(|p| (n.clone(), p, body))
        }
        _ => None,
    }
}

pub fn new_id() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("ws-{t:x}")
}

pub fn list_workspaces(dir: &Path) -> Vec<Value> {
    let mut out: Vec<(std::time::SystemTime, Value)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p: PathBuf = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(t) = std::fs::read(&p) else { continue };
            let Ok(v) = serde_json::from_slice::<Value>(&t) else { continue };
            let mt = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            let rows = v["rows"].as_array().map(|r| r.iter().filter(|x| !x["text"].as_str().unwrap_or("").is_empty()).count()).unwrap_or(0);
            let secs = mt.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            out.push((mt, json!({"id": v["id"], "name": v["name"], "rows": rows, "modified": secs})));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|x| x.1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Vec<Value> {
        let mut d = Doc::new("t".into(), "t".into());
        d.rows.clear();
        for l in lines {
            d.set_text(None, l);
        }
        d.compute(Opts { degrees: false, digits: 10 }).as_array().unwrap().clone()
    }

    #[test]
    fn rows_and_definitions() {
        let r = run(&["b = a + 1", "a = 2", "f(x) = x^2 + b", "f(3)", "1/3 + 1/6", "y = x^2", "c + 1", "2^100"]);
        assert_eq!(r[0]["value"], "3");
        assert_eq!(r[1]["value"], "2");
        assert_eq!(r[2]["kind"], "func");
        assert_eq!(r[3]["value"], "12");
        assert_eq!(r[4]["value"], "1/2");
        assert_eq!(r[4]["approx"], "0.5");
        assert_eq!(r[4]["exact"], true);
        assert_eq!(r[5]["kind"], "graph");
        assert_eq!(r[6]["free"][0], "c");
        assert_eq!(r[7]["value"], "1267650600228229401496703205376");
    }

    #[test]
    fn matrices() {
        let r = run(&["A = [[2,1],[1,3]]", "det(A)", "A^-1", "A^T", "A*[[1],[0]]", "rank([1,2;2,4])", "det([[1,2],[3]])"]);
        assert_eq!(r[1]["value"], "5");
        assert_eq!(r[2]["value"], "[3/5, -1/5; -1/5, 2/5]");
        assert_eq!(r[3]["value"], "[2, 1; 1, 3]");
        assert_eq!(r[4]["value"], "[2; 1]");
        assert_eq!(r[5]["value"], "1");
        assert_eq!(r[6]["kind"], "error");
    }

    #[test]
    fn functions_and_trig() {
        let r = run(&["sqrt(8)", "sqrt(16/9)", "sin(pi/6)", "log_2 8", "5!", "nCr(10,3)", "|-3|+|2-5|", "gcd(12,18)", "(1,2)+(3,4)"]);
        assert_eq!(r[1]["value"], "4/3");
        assert_eq!(r[2]["value"], "1/2");
        assert_eq!(r[3]["value"], "3");
        assert_eq!(r[4]["value"], "120");
        assert_eq!(r[5]["value"], "120");
        assert_eq!(r[6]["value"], "6");
        assert_eq!(r[7]["value"], "6");
        assert_eq!(r[8]["value"], "(4, 6)");
    }
}
