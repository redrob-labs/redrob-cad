//! Ask for what a command is missing. A command without its own prompt machine, typed at the
//! command line with no parameters, used to fail with "invalid parameters … `name` is required".
//! AutoCAD asks instead, so this does too: it reads which parameter the command wants, prompts
//! for it (or for objects), and runs the command again with the answer.

use redrob_cad_doc::Handle;
use serde_json::{Map, Value, json};

use super::machines::{SelOutcome, SelectPhase};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most questions one command may ask before giving up.
const MAX_ASKS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Point,
    Number,
    Text,
    /// One object, stored as a handle string.
    Object,
    /// Objects, stored as a list of handle strings (or just left selected).
    Objects,
}

#[derive(Clone, Debug)]
struct Need {
    key: Option<String>,
    kind: Kind,
}

/// What a "missing parameter" error asks for, or `None` when it is not that kind of error.
fn need_from(msg: &str) -> Option<Need> {
    let lower = msg.to_ascii_lowercase();
    let key = msg.split('`').nth(1).map(str::to_string).filter(|k| !k.is_empty() && !k.contains(' '));
    let wants_selection = lower.contains("nothing selected") || lower.starts_with("select ") || lower.contains("no viewport given");
    if key.is_none() {
        return wants_selection.then_some(Need { key: None, kind: Kind::Objects });
    }
    if !lower.contains("is required") {
        return None;
    }
    let k = key.clone().unwrap_or_default();
    let kind = match k.as_str() {
        "handle" | "source" | "baseHandle" | "h1" => Kind::Object,
        "handles" | "targets" => Kind::Objects,
        _ if lower.contains("[x, y]") => Kind::Point,
        "at" | "center" | "base" | "min" | "max" | "p1" | "p2" | "vertex" => Kind::Point,
        _ if lower.contains("(number)") => Kind::Number,
        "scale" | "index" | "lineweight" | "color" => Kind::Number,
        _ if lower.contains("dimension") || lower.contains("of a table") => Kind::Object,
        _ => Kind::Text,
    };
    // "`base` dimension is required" names an object, not a point.
    let kind = if lower.contains("dimension is required") || lower.contains("of a table") { Kind::Object } else { kind };
    Some(Need { key, kind })
}

/// Words shown at the prompt, e.g. `layerState` -> "layer state".
fn spoken(key: &str) -> String {
    let mut s = String::new();
    for c in key.chars() {
        if c.is_ascii_uppercase() {
            s.push(' ');
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

pub(crate) struct AskM {
    id: &'static str,
    params: Map<String, Value>,
    need: Need,
    sel: SelectPhase,
    asked: usize,
}

impl AskM {
    /// A machine for `id` if `err` is a missing-parameter error it can answer.
    pub(crate) fn for_error(id: &'static str, err: &EngineError) -> Option<Self> {
        let EngineError::BadParams { msg, .. } = err else { return None };
        let need = need_from(msg)?;
        let sel = if need.kind == Kind::Object { SelectPhase::single() } else { SelectPhase::default() };
        Some(AskM { id, params: Map::new(), need, sel, asked: 1 })
    }

    fn store(&mut self, v: Value) {
        if let Some(k) = &self.need.key {
            self.params.insert(k.clone(), v);
        }
    }

    /// Run the command with what we have. Done on success, Continue with the next question.
    fn retry(&mut self, s: &mut Session) -> Result<Step> {
        let spec = super::find_command(self.id).ok_or_else(|| EngineError::UnknownCommand(self.id.into()))?;
        let p = Value::Object(self.params.clone());
        match (spec.run)(s, &p) {
            Ok(r) => {
                if let Some(t) = r.as_str() {
                    s.echo(t.to_string());
                } else if let Some(m) = r.get("message").and_then(Value::as_str) {
                    s.echo(m.to_string());
                }
                Ok(Step::Done)
            }
            Err(e) => {
                let next = match &e {
                    EngineError::BadParams { msg, .. } => need_from(msg),
                    _ => None,
                };
                match next {
                    Some(n) if self.asked < MAX_ASKS && !self.already_has(&n) => {
                        self.asked += 1;
                        self.sel = if n.kind == Kind::Object { SelectPhase::single() } else { SelectPhase::default() };
                        self.need = n;
                        Ok(Step::Continue)
                    }
                    _ => {
                        s.echo(plain(&e));
                        Ok(Step::Cancel)
                    }
                }
            }
        }
    }

    fn already_has(&self, n: &Need) -> bool {
        match &n.key {
            Some(k) => self.params.contains_key(k),
            None => false,
        }
    }
}

/// The error text without the "invalid parameters for `x`:" wrapper.
pub(crate) fn plain(e: &EngineError) -> String {
    match e {
        EngineError::BadParams { msg, .. } => msg.clone(),
        other => other.to_string(),
    }
}

impl Interactive for AskM {
    fn name(&self) -> &'static str {
        self.id
    }

    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        // A pickfirst selection already answers an "objects" question.
        if matches!(self.need.kind, Kind::Objects | Kind::Object) {
            let pre = s.selection();
            if !pre.is_empty() && s.settings.pickfirst {
                return self.picked(s, pre);
            }
        }
        Ok(Step::Continue)
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        let what = self.need.key.as_deref().map(spoken).unwrap_or_default();
        match self.need.kind {
            Kind::Point => Prompt::new(format!("Specify {what}"), Accept::POINT),
            Kind::Number => Prompt::new(format!("Enter {what}"), Accept::NUMBER),
            Kind::Text => Prompt::new(format!("Enter {what}"), Accept::TEXT),
            Kind::Object | Kind::Objects => self.sel.prompt(),
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.need.kind {
            Kind::Object | Kind::Objects => match self.sel.feed(s, &i)? {
                SelOutcome::More => Ok(Step::Continue),
                SelOutcome::Done(hs) => self.picked(s, hs),
                SelOutcome::Empty => Ok(Step::Cancel),
            },
            Kind::Point => match i {
                Input::Point(p) => {
                    self.store(json!([p.x, p.y]));
                    self.retry(s)
                }
                Input::Text(t) | Input::Keyword(t) => match crate::prompt::parse_point(&t, s.last_point) {
                    Some(p) => {
                        self.store(json!([p.x, p.y]));
                        self.retry(s)
                    }
                    None => {
                        s.echo("Invalid point.");
                        Ok(Step::Continue)
                    }
                },
                Input::Enter | Input::Cancel => Ok(Step::Cancel),
                Input::Pick(_) => Ok(Step::Continue),
            },
            Kind::Number | Kind::Text => match i {
                Input::Text(t) | Input::Keyword(t) => {
                    let t = t.trim().to_string();
                    if t.is_empty() {
                        return Ok(Step::Cancel);
                    }
                    let v = match t.parse::<f64>() {
                        Ok(f) if f.is_finite() && self.need.kind == Kind::Number => {
                            if f.fract() == 0.0 && f.abs() < 1e15 {
                                json!(f as i64)
                            } else {
                                json!(f)
                            }
                        }
                        _ if self.need.kind == Kind::Number => {
                            s.echo("Requires a number.");
                            return Ok(Step::Continue);
                        }
                        _ => json!(t),
                    };
                    self.store(v);
                    self.retry(s)
                }
                Input::Enter | Input::Cancel => Ok(Step::Cancel),
                Input::Point(_) | Input::Pick(_) => Ok(Step::Continue),
            },
        }
    }
}

impl AskM {
    fn picked(&mut self, s: &mut Session, hs: Vec<Handle>) -> Result<Step> {
        if hs.is_empty() {
            return Ok(Step::Cancel);
        }
        s.set_selection(hs.clone());
        match self.need.kind {
            Kind::Object => {
                if let Some(h) = hs.first() {
                    self.store(json!(h.hex()));
                }
            }
            _ => self.store(Value::Array(hs.iter().map(|h| json!(h.hex())).collect())),
        }
        self.retry(s)
    }
}

// ---------- system variables typed as commands ----------

/// Variables AutoCAD lets you type at "Command:" that this engine stores but does not list.
const EXTRA_VARS: &[&str] = &["HPDLGMODE"];

/// The upper-case variable name when `name` (typed at "Command:") is a system variable.
pub(crate) fn sysvar_name(s: &Session, name: &str) -> Option<String> {
    let n = name.trim().to_ascii_uppercase();
    if n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let known = crate::sysvars::get(s, &n).is_some() || EXTRA_VARS.contains(&n.as_str()) || dim_field(&n).is_some();
    known.then_some(n)
}

/// The dimension-style field a `DIM…` variable sets. DIMSCALE is a drawing variable here
/// (it multiplies every style), so it is not one of them.
fn dim_field(name: &str) -> Option<&'static str> {
    if name == "DIMSCALE" {
        return None;
    }
    redrob_cad_doc::DIMVARS.iter().find(|(v, _)| *v == name).map(|(_, f)| *f)
}

fn current_dimstyle(s: &Session) -> String {
    s.doc().map(|d| d.header.str("DIMSTYLE", "Standard")).unwrap_or_else(|_| "Standard".into())
}

fn show(v: &Value) -> String {
    match v {
        Value::Number(n) if n.is_f64() => format!("{:.4}", n.as_f64().unwrap_or(0.0)),
        Value::String(t) => t.clone(),
        Value::Array(a) => a.iter().map(show).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

/// "Enter new value for DIMSCALE <1.0000>:".
pub(crate) struct SysvarM {
    name: String,
}

impl SysvarM {
    pub(crate) fn new(name: String) -> Self {
        SysvarM { name }
    }

    fn current(&self, s: &Session) -> Option<Value> {
        match dim_field(&self.name) {
            Some(field) => {
                let d = s.doc().ok()?;
                let st = d.dim_style(&current_dimstyle(s))?;
                serde_json::to_value(st).ok()?.get(field).cloned()
            }
            None => crate::sysvars::get(s, &self.name),
        }
    }

    fn apply(&self, s: &mut Session, text: &str) -> Result<()> {
        let t = text.trim();
        let typed = match t.parse::<f64>() {
            Ok(f) if f.is_finite() => {
                if f.fract() == 0.0 && f.abs() < 1e15 && !t.contains('.') {
                    json!(f as i64)
                } else {
                    json!(f)
                }
            }
            _ => json!(t),
        };
        match dim_field(&self.name) {
            Some(field) => {
                let spec = super::find_command("dimstyle").ok_or_else(|| EngineError::UnknownCommand("dimstyle".into()))?;
                let mut p = Map::new();
                p.insert("name".into(), json!(current_dimstyle(s)));
                p.insert(field.into(), typed);
                let r = (spec.run)(s, &Value::Object(p))?;
                if r.get("ignored").is_some() {
                    return Err(EngineError::Other(format!("Invalid value for {}.", self.name)));
                }
            }
            None => crate::sysvars::set(s, &self.name, &typed)?,
        }
        s.touch();
        Ok(())
    }
}

impl Interactive for SysvarM {
    fn name(&self) -> &'static str {
        "SETVAR"
    }

    fn prompt(&self, s: &Session) -> Prompt {
        let p = Prompt::new(format!("Enter new value for {}", self.name), Accept::TEXT);
        match self.current(s) {
            Some(v) => p.default(show(&v)),
            None => p,
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Text(t) | Input::Keyword(t) if !t.trim().is_empty() => {
                self.apply(s, &t)?;
                Ok(Step::Done)
            }
            Input::Point(p) => {
                self.apply(s, &format!("{},{}", p.x, p.y))?;
                Ok(Step::Done)
            }
            Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Done),
        }
    }
}
