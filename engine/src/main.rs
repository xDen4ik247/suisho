//! suisho-engine — the JSON-lines bridge between the QML window and the math engine.
//! One request per line on stdin: {"cmd": "...", "rid": N, ...}; one reply per line on stdout:
//! {"type": "...", "rid": N, ...}. Errors come back as {"type": "error", "error": "..."}.
//! Paths: workspaces in $SUISHO_DATA (~/.local/share/suisho/workspaces), settings in $SUISHO_CONFIG
//! (~/.config/suisho/settings.json).

mod algebra;
mod calc;
mod doc;
mod eval;
mod geom;
mod mtree;
mod num;
mod parse;
mod plot;
mod poly;
mod steps;
mod sym;
mod units;

use doc::{list_workspaces, new_id, Doc, Opts};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}
fn data_dir() -> PathBuf {
    std::env::var_os("SUISHO_DATA").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share/suisho"))
}
fn config_dir() -> PathBuf {
    std::env::var_os("SUISHO_CONFIG").map(PathBuf::from).unwrap_or_else(|| {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config")).join("suisho")
    })
}
fn ws_dir() -> PathBuf {
    data_dir().join("workspaces")
}

struct App {
    settings: Map<String, Value>,
    doc: Doc,
}

fn default_settings() -> Map<String, Value> {
    let v = json!({"lang": "en", "angle": "rad", "digits": 10, "last_ws": null, "ru_notation": false});
    v.as_object().unwrap().clone()
}

impl App {
    fn load() -> App {
        let mut settings = default_settings();
        if let Ok(t) = std::fs::read(config_dir().join("settings.json")) {
            if let Ok(Value::Object(m)) = serde_json::from_slice::<Value>(&t) {
                for (k, v) in m {
                    settings.insert(k, v);
                }
            }
        }
        let doc = settings
            .get("last_ws")
            .and_then(|v| v.as_str())
            .and_then(|id| std::fs::read(ws_dir().join(format!("{id}.json"))).ok())
            .and_then(|t| serde_json::from_slice::<Value>(&t).ok())
            .and_then(|v| Doc::from_json(&v))
            .unwrap_or_else(|| Doc::new(new_id(), "Untitled".into()));
        let mut app = App { settings, doc };
        app.remember_ws();
        app
    }

    fn opts(&self) -> Opts {
        Opts {
            degrees: self.settings.get("angle").and_then(|v| v.as_str()) == Some("deg"),
            digits: self.settings.get("digits").and_then(|v| v.as_u64()).unwrap_or(10).clamp(3, 17) as usize,
        }
    }

    fn save_settings(&self) {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let tmp = dir.join("settings.json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&Value::Object(self.settings.clone())).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, dir.join("settings.json"));
        }
    }

    fn remember_ws(&mut self) {
        if self.settings.get("last_ws").and_then(|v| v.as_str()) != Some(&self.doc.id) {
            self.settings.insert("last_ws".into(), json!(self.doc.id));
            self.save_settings();
        }
    }

    fn doc_reply(&mut self) -> Value {
        let rows = self.doc.compute(self.opts());
        if let Err(e) = self.doc.save(&ws_dir()) {
            eprintln!("suisho-engine: can't save the workspace: {e}");
        }
        json!({"type": "doc", "ws": {"id": self.doc.id, "name": self.doc.name}, "rows": rows})
    }

    fn handle(&mut self, r: &Map<String, Value>) -> Result<Value, String> {
        let cmd = r.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let id = r.get("id").and_then(|v| v.as_u64());
        Ok(match cmd {
            "hello" => {
                let doc = self.doc_reply();
                json!({"type": "hello", "version": VERSION, "settings": self.settings, "doc": doc,
                       "workspaces": list_workspaces(&ws_dir())})
            }
            "settings_set" => {
                if let Some(Value::Object(m)) = r.get("set") {
                    for (k, v) in m {
                        self.settings.insert(k.clone(), v.clone());
                    }
                    self.save_settings();
                }
                let doc = self.doc_reply();
                json!({"type": "settings", "settings": self.settings, "doc": doc})
            }
            "row_set" => {
                let text = r.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let new_id = self.doc.set_text(id, text);
                let mut d = self.doc_reply();
                d["focus"] = json!(new_id);
                d
            }
            "row_insert" => {
                let after = r.get("after").and_then(|v| v.as_u64());
                let text = r.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let new_id = self.doc.insert_after(after, text);
                let mut d = self.doc_reply();
                d["focus"] = json!(new_id);
                d
            }
            "row_del" => {
                self.doc.delete(id.ok_or("row_del needs an id")?);
                self.doc_reply()
            }
            "row_move" => {
                let to = r.get("to").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.doc.move_row(id.ok_or("row_move needs an id")?, to);
                self.doc_reply()
            }
            "row_hide" => {
                let h = r.get("hidden").and_then(|v| v.as_bool()).unwrap_or(true);
                self.doc.set_hidden(id.ok_or("row_hide needs an id")?, h);
                self.doc_reply()
            }
            "plot" => {
                let v = r.get("view").ok_or("plot needs a view")?;
                let g = |k: &str| v.get(k).and_then(|x| x.as_f64()).ok_or(format!("view.{k} missing"));
                let view = plot::View { x0: g("x0")?, x1: g("x1")?, y0: g("y0")?, y1: g("y1")?, w: g("w")?, h: g("h")? };
                if !(view.x1 > view.x0 && view.y1 > view.y0 && view.w >= 1.0 && view.h >= 1.0 && view.w <= 20000.0 && view.h <= 20000.0) {
                    return Err("bad view".into());
                }
                let mut out = self.doc.plot(self.opts(), view);
                out["type"] = json!("plot");
                out["view"] = v.clone();
                out
            }
            "mesh" => {
                let rr = r.get("r").and_then(|v| v.as_f64()).unwrap_or(5.0).clamp(1e-3, 1e6);
                let n = r.get("n").and_then(|v| v.as_u64()).unwrap_or(80) as usize;
                let mut out = self.doc.mesh(self.opts(), rr, n);
                out["type"] = json!("mesh");
                out
            }
            "steps" => {
                let id = id.ok_or("steps needs an id")?;
                let sol = self.doc.solution(id, self.opts())?;
                json!({"type": "steps", "id": id, "sol": sol})
            }
            "undo" | "redo" => {
                if cmd == "undo" {
                    self.doc.undo();
                } else {
                    self.doc.redo();
                }
                self.doc_reply()
            }
            "ws_list" => json!({"type": "ws_list", "workspaces": list_workspaces(&ws_dir())}),
            "ws_new" => {
                let _ = self.doc.save(&ws_dir());
                let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("Untitled");
                self.doc = Doc::new(new_id(), name.into());
                self.remember_ws();
                let mut d = self.doc_reply();
                d["workspaces"] = json!(list_workspaces(&ws_dir()));
                d
            }
            "ws_open" => {
                let wid = r.get("ws").and_then(|v| v.as_str()).ok_or("ws_open needs ws")?;
                let t = std::fs::read(ws_dir().join(format!("{wid}.json"))).map_err(|e| format!("can't open: {e}"))?;
                let v: Value = serde_json::from_slice(&t).map_err(|e| format!("damaged workspace: {e}"))?;
                let _ = self.doc.save(&ws_dir());
                self.doc = Doc::from_json(&v).ok_or("damaged workspace")?;
                self.remember_ws();
                self.doc_reply()
            }
            "ws_rename" => {
                let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("").trim();
                if !name.is_empty() {
                    self.doc.name = name.chars().take(80).collect();
                    self.doc.dirty = true;
                }
                let mut d = self.doc_reply();
                d["workspaces"] = json!(list_workspaces(&ws_dir()));
                d
            }
            "ws_delete" => {
                let wid = r.get("ws").and_then(|v| v.as_str()).ok_or("ws_delete needs ws")?;
                if wid.contains('/') || wid.contains("..") {
                    return Err("bad workspace id".into());
                }
                let _ = std::fs::remove_file(ws_dir().join(format!("{wid}.json")));
                if wid == self.doc.id {
                    self.doc = Doc::new(new_id(), "Untitled".into());
                    self.remember_ws();
                }
                let mut d = self.doc_reply();
                d["workspaces"] = json!(list_workspaces(&ws_dir()));
                d
            }
            c => return Err(format!("unknown cmd {c:?}")),
        })
    }
}

fn main() {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    let mut app = App::load();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: Option<Value> = serde_json::from_str(line.trim()).ok();
        let rid = req.as_ref().and_then(|r| r.get("rid").cloned()).unwrap_or(Value::Null);
        let res = match &req {
            Some(Value::Object(r)) if r.get("cmd").and_then(|v| v.as_str()) == Some("quit") => break,
            Some(Value::Object(r)) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.handle(r))) {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => json!({"type": "error", "error": e}),
                    Err(_) => json!({"type": "error", "error": "internal error (the engine caught a crash; please report the input)"}),
                }
            }
            _ => json!({"type": "error", "error": "bad request (not a JSON object)"}),
        };
        let mut res = res;
        res["rid"] = rid;
        if writeln!(out, "{res}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
    let _ = app.doc.save(&ws_dir());
}
