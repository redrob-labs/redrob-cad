//! Display order and visibility: DRAWORDER (Above/Under objects, Front/Back), TEXTTOFRONT,
//! HATCHTOBACK, ISOLATEOBJECTS / HIDEOBJECTS / UNISOLATEOBJECTS; and TXT2MTXT.

use cadcraft_doc::{EntityKind, Handle, MText};
use serde_json::{Value, json};

use super::machines::{SelOutcome, SelectPhase};
use super::*;
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("draworder", "Draw Order", run_draworder)
            .alias(&["dr"])
            .params("{handles?, order: above|under|front|back, reference?: [handles] (above/under)}")
            .interactive(|_| Ok(Box::new(DrawOrderM::default()))),
        CommandSpec::new("draworder.above", "Bring Above Objects", |s, p| run_draworder(s, &with(p, "order", "above")))
            .menu(&["Tools", "Draw Order", "Bring Above Objects"])
            .params("{handles?, reference: [handles]}")
            .interactive(|_| Ok(Box::new(DrawOrderM::with(Order::Above)))),
        CommandSpec::new("draworder.under", "Send Under Objects", |s, p| run_draworder(s, &with(p, "order", "under")))
            .menu(&["Tools", "Draw Order", "Send Under Objects"])
            .params("{handles?, reference: [handles]}")
            .interactive(|_| Ok(Box::new(DrawOrderM::with(Order::Under)))),
        CommandSpec::new("texttofront", "All Annotation Objects", |s, p| run_texttofront(s, p))
            .menu(&["Tools", "Draw Order", "Bring Annotations to Front", "All Annotation Objects"])
            .params("{kind?: text|dimensions|leaders|all}"),
        CommandSpec::new("texttofront.text", "Text Objects Only", |s, _| run_texttofront(s, &json!({ "kind": "text" })))
            .menu(&["Tools", "Draw Order", "Bring Annotations to Front", "Text Objects Only"]),
        CommandSpec::new("texttofront.dimensions", "Dimension Objects Only", |s, _| run_texttofront(s, &json!({ "kind": "dimensions" })))
            .menu(&["Tools", "Draw Order", "Bring Annotations to Front", "Dimension Objects Only"]),
        CommandSpec::new("texttofront.leaders", "Leaders Only", |s, _| run_texttofront(s, &json!({ "kind": "leaders" })))
            .menu(&["Tools", "Draw Order", "Bring Annotations to Front", "Leaders Only"]),
        CommandSpec::new("hatchtoback", "Send Hatches to Back", run_hatchtoback).menu(&["Tools", "Draw Order", "Send Hatches to Back"]),
        CommandSpec::new("isolateobjects", "Isolate Objects", |s, p| run_hide(s, p, true))
            .menu(&["Tools", "Isolate", "Isolate Objects"])
            .params("{handles?}")
            .interactive(|_| Ok(Box::new(HideM { isolate: true, sel: SelectPhase::default() }))),
        CommandSpec::new("hideobjects", "Hide Objects", |s, p| run_hide(s, p, false))
            .menu(&["Tools", "Isolate", "Hide Objects"])
            .params("{handles?}")
            .interactive(|_| Ok(Box::new(HideM { isolate: false, sel: SelectPhase::default() }))),
        CommandSpec::new("unisolateobjects", "End Object Isolation", run_unisolate).menu(&["Tools", "Isolate", "End Object Isolation"]),
        CommandSpec::new("txt2mtxt", "Convert Text to Mtext", run_txt2mtxt)
            .menu(&["Tools", "Express Tools", "Convert Text to Mtext"])
            .params("{handles?, combine?: bool (default true)}")
            .interactive(|_| Ok(Box::new(Txt2MtxtM { sel: SelectPhase::default() }))),
    ]
}

fn with(p: &Value, k: &str, v: &str) -> Value {
    let mut q = if p.is_object() { p.clone() } else { json!({}) };
    if let Some(o) = q.as_object_mut() {
        o.insert(k.into(), json!(v));
    }
    q
}

fn hex_list(p: &Value, key: &str) -> Vec<Handle> {
    p.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().and_then(Handle::parse_hex)).collect()).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Order {
    Above,
    Under,
    Front,
    Back,
}

fn order_of(s: &str) -> Option<Order> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "a" | "above" | "above objects" => Order::Above,
        "u" | "under" | "under objects" => Order::Under,
        "f" | "front" => Order::Front,
        "b" | "back" => Order::Back,
        _ => return None,
    })
}

/// Rewrite the space's draw order: `objs` (in their current relative order) moved to just
/// above the topmost / just below the lowest reference object, or to the very front / back.
fn reorder(s: &mut Session, objs: &[Handle], order: Order, refs: &[Handle]) -> Result<usize> {
    let space = s.space();
    let current: Vec<Handle> = s.doc()?.space(&space).map(|st| st.handles()).unwrap_or_default();
    let moving: Vec<Handle> = current.iter().copied().filter(|h| objs.contains(h)).collect();
    if moving.is_empty() {
        return Ok(0);
    }
    let rest: Vec<Handle> = current.iter().copied().filter(|h| !objs.contains(h)).collect();
    let at = match order {
        Order::Front => rest.len(),
        Order::Back => 0,
        Order::Above => rest.iter().rposition(|h| refs.contains(h)).map(|i| i + 1).ok_or_else(|| EngineError::Other("No reference objects.".into()))?,
        Order::Under => rest.iter().position(|h| refs.contains(h)).ok_or_else(|| EngineError::Other("No reference objects.".into()))?,
    };
    let mut next: Vec<Handle> = rest.get(..at).unwrap_or(&[]).to_vec();
    next.extend(&moving);
    next.extend(rest.get(at..).unwrap_or(&[]));
    if next == current {
        return Ok(moving.len());
    }
    let d = s.doc_mut()?;
    if let Some(st) = d.space_mut(&space) {
        // Rebuild in the new order: everything goes to the front in sequence.
        for h in &next {
            st.bring_to_front(*h);
        }
    }
    Ok(moving.len())
}

fn run_draworder(s: &mut Session, p: &Value) -> Result<Value> {
    let objs = targets(s, p)?;
    if objs.is_empty() {
        return Err(bad("draworder", "nothing selected"));
    }
    let order = str_param(p, "order").and_then(order_of).ok_or_else(|| bad("draworder", "`order` is required (above, under, front, back)"))?;
    let refs = hex_list(p, "reference");
    if matches!(order, Order::Above | Order::Under) && refs.is_empty() {
        return Err(bad("draworder", "`reference` is required"));
    }
    let n = reorder(s, &objs, order, &refs)?;
    Ok(json!({ "moved": n }))
}

fn kind_matches(k: &EntityKind, which: &str) -> bool {
    let text = matches!(k, EntityKind::Text(_) | EntityKind::MText(_));
    let dims = matches!(k, EntityKind::Dimension(_));
    let leaders = matches!(k, EntityKind::Leader(_) | EntityKind::MLeader(_));
    match which {
        "text" => text,
        "dimensions" | "dim" => dims,
        "leaders" => leaders,
        _ => text || dims || leaders,
    }
}

fn run_texttofront(s: &mut Session, p: &Value) -> Result<Value> {
    let which = str_param(p, "kind").unwrap_or("all").to_ascii_lowercase();
    let space = s.space();
    let hs: Vec<Handle> =
        s.doc()?.space(&space).map(|st| st.iter().filter(|e| kind_matches(&e.kind, &which)).map(|e| e.handle).collect()).unwrap_or_default();
    let n = reorder(s, &hs, Order::Front, &[])?;
    Ok(json!({ "moved": n, "message": format!("{n} object(s) brought to front.") }))
}

fn run_hatchtoback(s: &mut Session, _p: &Value) -> Result<Value> {
    let space = s.space();
    let hs: Vec<Handle> =
        s.doc()?.space(&space).map(|st| st.iter().filter(|e| matches!(e.kind, EntityKind::Hatch(_))).map(|e| e.handle).collect()).unwrap_or_default();
    let n = reorder(s, &hs, Order::Back, &[])?;
    Ok(json!({ "moved": n, "message": format!("{n} hatch object(s) sent to back.") }))
}

/// HIDEOBJECTS hides the selection; ISOLATEOBJECTS hides everything else in the space.
fn run_hide(s: &mut Session, p: &Value, isolate: bool) -> Result<Value> {
    let sel = targets(s, p)?;
    let id = if isolate { "isolateobjects" } else { "hideobjects" };
    if sel.is_empty() {
        return Err(bad(id, "nothing selected"));
    }
    let space = s.space();
    let hide: Vec<Handle> = s
        .doc()?
        .space(&space)
        .map(|st| st.iter().filter(|e| e.common.visible && (sel.contains(&e.handle) != isolate)).map(|e| e.handle).collect())
        .unwrap_or_default();
    let d = s.doc_mut()?;
    for h in &hide {
        d.modify_entity(*h, |e| e.common.visible = false)?;
    }
    let st = s.state_mut()?;
    for h in &hide {
        if !st.isolated.contains(h) {
            st.isolated.push(*h);
        }
    }
    s.set_selection(Vec::new());
    Ok(json!({ "hidden": hide.len(), "message": format!("{} object(s) {}.", if isolate { sel.len() } else { hide.len() }, if isolate { "isolated" } else { "hidden" }) }))
}

fn run_unisolate(s: &mut Session, _p: &Value) -> Result<Value> {
    let hs = std::mem::take(&mut s.state_mut()?.isolated);
    if hs.is_empty() {
        return Ok(json!({ "shown": 0, "message": "No objects are isolated or hidden." }));
    }
    let d = s.doc_mut()?;
    let mut n = 0;
    for h in &hs {
        if d.modify_entity(*h, |e| e.common.visible = true).is_ok() {
            n += 1;
        }
    }
    Ok(json!({ "shown": n, "message": format!("{n} object(s) shown.") }))
}

/// TXT2MTXT: single-line texts become one multiline text (top line first), or one each.
fn run_txt2mtxt(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let d = s.doc()?;
    let mut texts: Vec<(Handle, cadcraft_doc::Text, cadcraft_doc::Common)> = hs
        .iter()
        .filter_map(|h| {
            let e = d.entity(*h)?;
            match &e.kind {
                EntityKind::Text(t) => Some((*h, t.clone(), e.common.clone())),
                _ => None,
            }
        })
        .collect();
    if texts.is_empty() {
        return Err(bad("txt2mtxt", "select text objects"));
    }
    // Reading order: top to bottom, then left to right.
    texts.sort_by(|a, b| b.1.insert.y.total_cmp(&a.1.insert.y).then(a.1.insert.x.total_cmp(&b.1.insert.x)));
    let combine = bool_or(p, "combine", true);
    let make = |ts: &[&(Handle, cadcraft_doc::Text, cadcraft_doc::Common)]| -> Option<EntityKind> {
        let first = ts.first()?;
        let h = first.1.height.max(1e-9);
        let x0 = ts.iter().map(|t| t.1.insert.x).fold(f64::INFINITY, f64::min);
        let contents = ts.iter().map(|t| t.1.value.replace('\\', "\\\\")).collect::<Vec<_>>().join("\\P");
        let spacing = if ts.len() > 1 {
            let gaps: Vec<f64> = ts.windows(2).filter_map(|w| Some(w.first()?.1.insert.y - w.get(1)?.1.insert.y)).collect();
            (gaps.iter().sum::<f64>() / gaps.len().max(1) as f64 / (h * 5.0 / 3.0)).clamp(0.25, 4.0)
        } else {
            1.0
        };
        Some(EntityKind::MText(MText {
            insert: cadcraft_geom::Vec3::new(x0, first.1.insert.y + h, 0.0),
            height: h,
            width: 0.0,
            attach: 1,
            rotation: first.1.rotation,
            style: first.1.style.clone(),
            contents,
            line_spacing: spacing,
        }))
    };
    let groups: Vec<Vec<&(Handle, cadcraft_doc::Text, cadcraft_doc::Common)>> =
        if combine { vec![texts.iter().collect()] } else { texts.iter().map(|t| vec![t]).collect() };
    let space = s.space();
    let mut out = Vec::new();
    for g in groups {
        let Some(kind) = make(&g) else { continue };
        let common = g.first().map(|t| t.2.clone()).unwrap_or_default();
        let d = s.doc_mut()?;
        for t in &g {
            d.remove_entity(t.0);
        }
        let nh = d.new_handle();
        if let Some(st) = d.space_mut(&space) {
            st.push(cadcraft_doc::Entity { handle: nh, common, kind });
        }
        out.push(nh.hex());
    }
    s.set_selection(Vec::new());
    Ok(json!({ "handles": out, "message": format!("{} text object(s) converted.", texts.len()) }))
}

// ---------- prompts ----------

#[derive(Default)]
struct DrawOrderM {
    sel: SelectPhase,
    objs: Vec<Handle>,
    fixed: Option<Order>,
    order: Option<Order>,
    refs: SelectPhase,
}

impl DrawOrderM {
    fn with(o: Order) -> Self {
        DrawOrderM { fixed: Some(o), ..Default::default() }
    }
    fn finish(&mut self, s: &mut Session, refs: &[Handle]) -> Result<Step> {
        let order = self.order.unwrap_or(Order::Back);
        match reorder(s, &self.objs, order, refs) {
            Ok(_) => {}
            Err(e) => s.echo(super::ask::plain(&e)),
        }
        s.set_selection(Vec::new());
        Ok(Step::Done)
    }
    fn chose(&mut self, s: &mut Session, o: Order) -> Result<Step> {
        self.order = Some(o);
        if matches!(o, Order::Above | Order::Under) {
            s.set_selection(Vec::new());
            return Ok(Step::Continue);
        }
        self.finish(s, &[])
    }
}

impl Interactive for DrawOrderM {
    fn name(&self) -> &'static str {
        "DRAWORDER"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.sel = SelectPhase::begin(s);
        if self.sel.done {
            self.objs = self.sel.picked.clone();
            if let Some(o) = self.fixed {
                return self.chose(s, o);
            }
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if !self.sel.done {
            return self.sel.prompt();
        }
        match self.order {
            None => Prompt::new("Enter object ordering option", Accept::TEXT).kw(&["Above objects", "Under objects", "Front", "Back"]).default("Back"),
            Some(_) => Prompt::new("Select reference objects", Accept::SELECT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if !self.sel.done {
            return match self.sel.feed(s, &i)? {
                SelOutcome::More => Ok(Step::Continue),
                SelOutcome::Empty => Ok(Step::Done),
                SelOutcome::Done(hs) => {
                    self.objs = hs;
                    match self.fixed {
                        Some(o) => self.chose(s, o),
                        None => Ok(Step::Continue),
                    }
                }
            };
        }
        if self.order.is_none() {
            return match i {
                Input::Keyword(k) | Input::Text(k) => match order_of(&k) {
                    Some(o) => self.chose(s, o),
                    None => {
                        s.echo("Invalid option keyword.");
                        Ok(Step::Continue)
                    }
                },
                Input::Enter => self.chose(s, Order::Back),
                Input::Cancel => Ok(Step::Cancel),
                _ => Ok(Step::Continue),
            };
        }
        match self.refs.feed(s, &i)? {
            SelOutcome::More => Ok(Step::Continue),
            SelOutcome::Empty => Ok(Step::Done),
            SelOutcome::Done(hs) => self.finish(s, &hs),
        }
    }
}

struct HideM {
    isolate: bool,
    sel: SelectPhase,
}

impl HideM {
    fn run(&self, s: &mut Session, hs: Vec<Handle>) -> Result<Step> {
        let r = run_hide(s, &json!({ "handles": hs.iter().map(|h| h.hex()).collect::<Vec<_>>() }), self.isolate)?;
        if let Some(m) = r.get("message").and_then(Value::as_str) {
            s.echo(m.to_string());
        }
        Ok(Step::Done)
    }
}

impl Interactive for HideM {
    fn name(&self) -> &'static str {
        if self.isolate { "ISOLATEOBJECTS" } else { "HIDEOBJECTS" }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.sel = SelectPhase::begin(s);
        if self.sel.done {
            let hs = self.sel.picked.clone();
            return self.run(s, hs);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        self.sel.prompt()
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.sel.feed(s, &i)? {
            SelOutcome::More => Ok(Step::Continue),
            SelOutcome::Empty => Ok(Step::Done),
            SelOutcome::Done(hs) => self.run(s, hs),
        }
    }
}

struct Txt2MtxtM {
    sel: SelectPhase,
}

impl Interactive for Txt2MtxtM {
    fn name(&self) -> &'static str {
        "TXT2MTXT"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.sel = SelectPhase::begin(s);
        if self.sel.done {
            let hs = self.sel.picked.clone();
            let r = run_txt2mtxt(s, &json!({ "handles": hs.iter().map(|h| h.hex()).collect::<Vec<_>>() }));
            if let Err(e) = r {
                s.echo(super::ask::plain(&e));
            }
            return Ok(Step::Done);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        self.sel.prompt()
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.sel.feed(s, &i)? {
            SelOutcome::More => Ok(Step::Continue),
            SelOutcome::Empty => Ok(Step::Done),
            SelOutcome::Done(hs) => {
                if let Err(e) = run_txt2mtxt(s, &json!({ "handles": hs.iter().map(|h| h.hex()).collect::<Vec<_>>() })) {
                    s.echo(super::ask::plain(&e));
                }
                Ok(Step::Done)
            }
        }
    }
}
