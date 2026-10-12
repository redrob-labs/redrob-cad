//! Korean coverage gates. A separate test binary, so switching the global language here cannot
//! race the library's unit tests.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cadcraft_ui_egui::i18n::{self, Lang, placeholders, tr, tr_keyword, tr_label, tr_log, tr_runtime};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Non-test source of every `.rs` file under `dir`.
fn sources(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") && !p.to_string_lossy().ends_with("tests.rs") {
                let s = std::fs::read_to_string(&p).unwrap_or_default();
                let s = match s.find("#[cfg(test)]") {
                    Some(i) => s[..i].to_string(),
                    None => s,
                };
                out.push((p, s));
            }
        }
    }
    out.sort();
    out
}

/// The string literal starting right after `open` (a `"`), unescaped; and where it ends.
fn literal(s: &str, open: usize) -> Option<(String, usize)> {
    let b = s.as_bytes();
    let mut i = open + 1;
    let mut out = String::new();
    while i < b.len() {
        match b[i] {
            b'"' => return Some((out, i + 1)),
            b'\\' if i + 1 < b.len() => {
                match b[i + 1] {
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    c => out.push(c as char),
                }
                i += 2;
            }
            _ => {
                let ch = s[i..].chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

/// Literal first arguments of calls to `name(` in `src`.
fn call_literals(src: &str, name: &str) -> Vec<String> {
    let pat = format!("{name}(\"");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = src[from..].find(&pat) {
        let at = from + i;
        let prev = src[..at].chars().next_back();
        let ok = prev.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        from = at + pat.len();
        if ok && let Some((l, _)) = literal(src, at + name.len() + 1) {
            out.push(l);
        }
    }
    out
}

fn ko_has(s: &str) -> bool {
    i18n::ko_table().contains_key(s)
}

#[test]
fn every_translation_keeps_its_slots() {
    let bad: Vec<_> = i18n::ko_table().iter().filter(|(en, ko)| placeholders(en) != placeholders(ko)).map(|(en, _)| en.clone()).collect();
    assert!(bad.is_empty(), "slots differ: {bad:?}");
}

#[test]
fn bracketed_keyword_options_stay_english() {
    // The user types these words; a translated list would show options the engine rejects.
    let bad: Vec<_> = i18n::ko_table()
        .iter()
        .filter(|(en, ko)| {
            let opts = |s: &str| s.split_once(" [").and_then(|(_, r)| r.split_once(']')).map(|(o, _)| o.to_string());
            opts(en).is_some() && opts(en) != opts(ko)
        })
        .map(|(en, _)| en.clone())
        .collect();
    assert!(bad.is_empty(), "translated keyword lists: {bad:?}");
}

#[test]
fn every_literal_passed_to_tr_has_korean() {
    let mut missing = BTreeSet::new();
    for dir in ["crates/ui-egui/src", "apps/cadcraft/src"] {
        for (path, src) in sources(&root().join(dir)) {
            if path.ends_with("i18n.rs") {
                continue;
            }
            for f in ["tr", "trf", "tr_label"] {
                for l in call_literals(&src, f) {
                    let english_only = matches!(l.as_str(), "Redrob CAD" | "Redrob Code" | "B" | "Y" | "github.com/redrob-labs/redrob-cad");
                    if !english_only && l.chars().any(|c| c.is_ascii_alphabetic()) && !ko_has(&l) {
                        missing.insert(format!("{}: {l:?}", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "{} strings have no Korean:\n{}", missing.len(), missing.into_iter().collect::<Vec<_>>().join("\n"));
}

/// New UI text must go through `tr`: a literal handed straight to an egui text call is English
/// forever. (Data tables are translated at their display site; this catches the direct case.)
#[test]
fn no_display_call_takes_a_raw_literal() {
    let calls = [
        "ui.label",
        ".button",
        "small_button",
        "Button::new",
        "hint_text",
        "on_hover_text",
        "RichText::new",
        "menu_button",
        "ui.heading",
        "Window::new",
        "hyperlink_to",
    ];
    let allowed = ["Redrob CAD", "▼", "×", "/", "]", ""];
    let mut raw = Vec::new();
    for (path, src) in sources(&root().join("crates/ui-egui/src")) {
        for c in calls {
            for l in call_literals(&src, c) {
                if !allowed.contains(&l.as_str()) && l.chars().any(|ch| ch.is_alphabetic()) {
                    raw.push(format!("{}: {c}({l:?})", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
                }
            }
        }
    }
    assert!(raw.is_empty(), "wrap these in tr():\n{}", raw.join("\n"));
}

#[test]
fn every_menu_entry_and_command_label_has_korean() {
    let app = cadcraft_ui_egui::CadApp::new(cadcraft_engine::Session::empty(), cadcraft_ui_egui::Services::default());
    let mut missing = BTreeSet::new();
    fn walk(e: &cadcraft_ui_egui::menus::Entry, missing: &mut BTreeSet<String>) {
        use cadcraft_ui_egui::menus::Entry;
        let label = match e {
            Entry::Item { label, .. } | Entry::Sub { label, .. } => label,
        };
        let base = label.strip_suffix("...").unwrap_or(label);
        if !ko_has(label) && !ko_has(base) && !matches!(label.as_str(), "English" | "한국어" | "ViewCube") {
            missing.insert(label.clone());
        }
        if let Entry::Sub { children, .. } = e {
            for c in children {
                walk(c, missing);
            }
        }
    }
    for (top, entries) in cadcraft_ui_egui::menus::tree(&app) {
        if !ko_has(&top) {
            missing.insert(top);
        }
        for e in &entries {
            walk(e, &mut missing);
        }
    }
    for c in cadcraft_engine::command_specs() {
        if !ko_has(c.label) && !ko_has(c.label.trim_end_matches("...")) {
            missing.insert(format!("command {}: {}", c.id, c.label));
        }
    }
    assert!(missing.is_empty(), "{} menu/command labels have no Korean:\n{}", missing.len(), missing.into_iter().collect::<Vec<_>>().join("\n"));
}

#[test]
fn every_literal_prompt_and_keyword_has_korean() {
    let mut missing = BTreeSet::new();
    for (path, src) in sources(&root().join("crates/engine/src")) {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for l in call_literals(&src, "Prompt::new") {
            if !l.is_empty() && !ko_has(&l) {
                missing.insert(format!("{name}: prompt {l:?}"));
            }
        }
        let mut from = 0;
        while let Some(i) = src[from..].find(".kw(&[") {
            let start = from + i + 6;
            let end = src[start..].find(']').map_or(src.len(), |j| start + j);
            let mut k = start;
            while let Some(q) = src[k..end].find('"') {
                if let Some((l, next)) = literal(&src, k + q) {
                    // All-capital keywords (ON, OFF) and single letters read the same in Korean CAD.
                    if !l.chars().all(|c| c.is_ascii_uppercase() || !c.is_alphabetic()) && !ko_has(&l) {
                        missing.insert(format!("{name}: keyword {l:?}"));
                    }
                    k = next;
                } else {
                    break;
                }
            }
            from = end;
        }
    }
    assert!(missing.is_empty(), "{} prompts/keywords have no Korean:\n{}", missing.len(), missing.into_iter().collect::<Vec<_>>().join("\n"));
}

#[test]
fn korean_lookups_behave() {
    i18n::set_lang(Lang::Ko);
    assert_eq!(tr("Send"), "보내기");
    assert_eq!(tr("No such string, untranslated"), "No such string, untranslated");
    assert_eq!(tr_label("Open..."), "열기...");
    assert_eq!(tr_keyword("eXit"), "종료(X)");
    assert_eq!(tr_keyword("Undo"), "명령 취소(U)");
    assert_eq!(tr_keyword("Nonexistent"), "Nonexistent");
    assert_eq!(tr_runtime("Model: redrob/auto"), "모델: redrob/auto");
    assert_eq!(tr_runtime("Workspace: Acme"), "워크스페이스: Acme");
    assert_eq!(tr_log("Command: CIRCLE"), "명령: CIRCLE");
    i18n::set_lang(Lang::En);
    assert_eq!(tr("Send"), "Send");
    assert_eq!(tr_runtime("Model: redrob/auto"), "Model: redrob/auto");
}

#[test]
fn language_codes_and_detection() {
    assert_eq!(Lang::from_code("ko_KR.UTF-8"), Some(Lang::Ko));
    assert_eq!(Lang::from_code("ko"), Some(Lang::Ko));
    assert_eq!(Lang::from_code("en-US"), Some(Lang::En));
    assert_eq!(Lang::from_code("C"), None);
    assert_eq!(Lang::from_code("C.UTF-8"), None);
    assert_eq!(Lang::from_code(""), None);
}
