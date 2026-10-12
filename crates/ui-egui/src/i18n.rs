//! Interface language.
//!
//! English is the source: every user-visible string is written in English in the code and passed
//! through [`tr`] where it is shown. [`tr`] looks the English text up in the active language's
//! table and falls back to it, so a missing entry shows English, never a blank or a key.
//!
//! Only presentation is translated. Command names (`CIRCLE`), keyword options typed at the command
//! line (`[Undo/Close]`), menu paths used as identifiers, MCP tool output and drawing data stay
//! English: users and agents type them and files store them.
//!
//! Tables live in `locales/<lang>.json` (English text -> translation). `tests` checks that every
//! string the UI shows has an entry and that `{placeholders}` match.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ko,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Ko];

    /// BCP 47 code, as stored in settings and read from `REDROB_CAD_LANG`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
        }
    }

    /// The language's own name, as the language menu shows it.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ko => "한국어",
        }
    }

    /// `ko`, `ko-KR`, `ko_KR.UTF-8` -> Korean; anything else English.
    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().to_ascii_lowercase();
        // `C.UTF-8` and `POSIX` say nothing about the language: fall through to the next source.
        let base = c.split(['.', '@']).next().unwrap_or("");
        if base.is_empty() || base == "c" || base == "posix" {
            return None;
        }
        Some(if c.starts_with("ko") { Lang::Ko } else { Lang::En })
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Ko,
        _ => Lang::En,
    }
}

pub fn set_lang(l: Lang) {
    LANG.store(if l == Lang::Ko { 1 } else { 0 }, Ordering::Relaxed);
}

const KO_JSON: &str = include_str!("../locales/ko.json");

fn ko() -> &'static HashMap<String, String> {
    static T: OnceLock<HashMap<String, String>> = OnceLock::new();
    T.get_or_init(|| parse(KO_JSON))
}

/// A table: a flat JSON object. A malformed table is a build-time bug the tests catch; at run
/// time it degrades to English rather than taking the app down.
fn parse(json: &str) -> HashMap<String, String> {
    match serde_json::from_str::<HashMap<String, String>>(json) {
        Ok(m) => m.into_iter().filter(|(_, v)| !v.is_empty()).collect(),
        Err(e) => {
            log::error!("locale table: {e}");
            HashMap::new()
        }
    }
}

/// `s` in the active language, or `s` itself.
pub fn tr(s: &str) -> &str {
    match lang() {
        Lang::En => s,
        Lang::Ko => ko().get(s).map(String::as_str).unwrap_or(s),
    }
}

/// `tr` for text with named slots: `trf("Workspace: {name}", &[("name", n)])`.
pub fn trf(s: &str, args: &[(&str, &str)]) -> String {
    let mut out = tr(s).to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// A menu path segment or command label, which may carry a trailing `...`.
pub fn tr_label(s: &str) -> std::borrow::Cow<'_, str> {
    if lang() == Lang::En {
        return s.into();
    }
    if let Some(t) = ko().get(s) {
        return t.as_str().into();
    }
    match s.strip_suffix("...") {
        Some(base) => match ko().get(base) {
            Some(t) => format!("{t}...").into(),
            None => s.into(),
        },
        None => s.into(),
    }
}

/// A keyword option for display: its translation with the shortcut letters the user types, as
/// `종료(X)` for `eXit`. The engine still receives the English keyword.
pub fn tr_keyword(k: &str) -> String {
    let t = tr(k);
    // Untranslated, or the translation already names what to type (`Ttr(접선 접선 반지름)`).
    if t == k || t.chars().any(|c| c.is_ascii_alphabetic()) {
        return t.to_string();
    }
    let keys: String = k.chars().filter(|c| c.is_ascii_uppercase()).collect();
    let keys = if keys.is_empty() { k.chars().next().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default() } else { keys };
    format!("{t}({keys})")
}

/// A command-line history line for display. The log itself stays English (agents read it); known
/// pieces are swapped: a whole line, the `Command:` prefix, or a known prompt before `: value`.
pub fn tr_log(line: &str) -> String {
    if lang() == Lang::En {
        return line.to_string();
    }
    // Reports (LIST, AREA) are one entry of many lines; each line keeps its indent.
    if line.contains('\n') {
        return line.split('\n').map(tr_log).collect::<Vec<_>>().join("\n");
    }
    let body = line.trim_start();
    if body.len() != line.len() && !body.is_empty() {
        return format!("{}{}", &line[..line.len() - body.len()], tr_log(body));
    }
    if let Some(t) = ko().get(line) {
        return t.clone();
    }
    if let Some(rest) = line.strip_prefix("Command:") {
        return format!("{}{rest}", tr("Command:"));
    }
    if let Some((head, value)) = line.split_once(": ")
        && let Some(t) = ko().get(head)
    {
        return format!("{t}: {value}");
    }
    tr_runtime(line)
}

/// Text built at run time (`format!` in the engine or the host): an exact entry, else an entry
/// whose `{slots}` match the variable parts, as `Model: {model}` matches `Model: redrob/auto`.
pub fn tr_runtime(s: &str) -> String {
    if lang() == Lang::En {
        return s.to_string();
    }
    let table = ko();
    if let Some(t) = table.get(s) {
        return t.clone();
    }
    for t in templates() {
        if let Some(vals) = match_template(&t.pieces, s) {
            let mut out = t.ko.clone();
            for (name, v) in t.names.iter().zip(vals) {
                out = out.replace(&format!("{{{name}}}"), v);
            }
            return out;
        }
    }
    s.to_string()
}

struct Template {
    /// The English text split at its slots: literal, slot, literal, ... (always odd length).
    pieces: Vec<String>,
    names: Vec<String>,
    ko: String,
}

fn templates() -> &'static [Template] {
    static T: OnceLock<Vec<Template>> = OnceLock::new();
    T.get_or_init(|| {
        let mut v: Vec<Template> = ko()
            .iter()
            .filter_map(|(en, k)| {
                let (pieces, names) = split_slots(en);
                (!names.is_empty()).then(|| Template { pieces, names, ko: k.clone() })
            })
            .collect();
        // Most literal text first, so the most specific template wins.
        v.sort_by_key(|t| std::cmp::Reverse(t.pieces.iter().map(String::len).sum::<usize>()));
        v
    })
}

fn split_slots(s: &str) -> (Vec<String>, Vec<String>) {
    let mut pieces = vec![String::new()];
    let mut names = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j) if !after[..j].is_empty() && after[..j].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
                if let Some(last) = pieces.last_mut() {
                    last.push_str(&rest[..i]);
                }
                names.push(after[..j].to_string());
                pieces.push(String::new());
                rest = &after[j + 1..];
            }
            _ => {
                if let Some(last) = pieces.last_mut() {
                    last.push_str(&rest[..=i]);
                }
                rest = after;
            }
        }
    }
    if let Some(last) = pieces.last_mut() {
        last.push_str(rest);
    }
    (pieces, names)
}

/// The slot values if `s` is `pieces[0] v0 pieces[1] v1 ... pieces[n]`; each slot is non-empty.
fn match_template<'a>(pieces: &[String], s: &'a str) -> Option<Vec<&'a str>> {
    let first = pieces.first()?;
    let mut rest = s.strip_prefix(first.as_str())?;
    let mut vals = Vec::new();
    for (k, lit) in pieces.iter().enumerate().skip(1) {
        let last = k + 1 == pieces.len();
        let at = if last {
            if lit.is_empty() { Some(rest.len()) } else { rest.ends_with(lit.as_str()).then(|| rest.len() - lit.len()) }
        } else if lit.is_empty() {
            return None; // two adjacent slots are ambiguous
        } else {
            rest.find(lit.as_str())
        }?;
        if at == 0 {
            return None;
        }
        vals.push(&rest[..at]);
        rest = &rest[at + lit.len()..];
    }
    rest.is_empty().then_some(vals)
}

/// The language the platform asks for: `REDROB_CAD_LANG`, then the POSIX locale variables.
/// (macOS and Windows set no `LANG` for GUI apps; the desktop app passes the OS locale in.)
pub fn detect(os_locale: Option<&str>) -> Lang {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    env("REDROB_CAD_LANG")
        .and_then(|v| Lang::from_code(&v))
        .or_else(|| env("LC_ALL").or_else(|| env("LC_MESSAGES")).or_else(|| env("LANG")).and_then(|v| Lang::from_code(&v)))
        .or_else(|| os_locale.and_then(Lang::from_code))
        .unwrap_or(Lang::En)
}

/// Every Korean entry, for the coverage tests.
pub fn ko_table() -> &'static HashMap<String, String> {
    ko()
}

/// `{name}` slots in a string, sorted.
pub fn placeholders(s: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        rest = &rest[i + 1..];
        match rest.find('}') {
            Some(j) => {
                let name = &rest[..j];
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    v.push(name.to_string());
                }
                rest = &rest[j + 1..];
            }
            None => break,
        }
    }
    v.sort();
    v
}
