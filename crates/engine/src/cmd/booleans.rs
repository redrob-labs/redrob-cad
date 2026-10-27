//! UNION, SUBTRACT, INTERSECT on regions: closed polylines (what REGION makes), circles, closed
//! ellipses and splines. The result replaces the inputs as closed polylines; arcs stay arcs and
//! a hole is a separate closed polyline inside its outline.

use redrob_cad_doc::{Entity, EntityKind, Handle};
use redrob_cad_geom::boolean::{BoolOp, Loop, boolean};
use redrob_cad_geom::{Arc, Line, PI, PolyVertex, Polyline, Segment, Vec2, arc_to_bulge};
use serde_json::{Value, json};

use super::helpers::lwpoly;
use super::machines::{SelOutcome, SelectPhase};
use super::*;
use crate::{EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("union", "Union", |s, p| run_bool(s, p, BoolOp::Union))
            .menu(&["Modify", "Solid Editing", "Union"])
            .alias(&["uni"])
            .params("{handles?} (regions: closed polylines, circles, closed ellipses/splines)")
            .interactive(|_| Ok(Box::new(BoolM::new(BoolOp::Union)))),
        CommandSpec::new("subtract", "Subtract", run_subtract)
            .menu(&["Modify", "Solid Editing", "Subtract"])
            .alias(&["su"])
            .params("{from: [handles], subtract: [handles]}")
            .interactive(|_| Ok(Box::new(BoolM::new(BoolOp::Subtract)))),
        CommandSpec::new("intersect", "Intersect", |s, p| run_bool(s, p, BoolOp::Intersect))
            .menu(&["Modify", "Solid Editing", "Intersect"])
            .alias(&["in"])
            .params("{handles?}")
            .interactive(|_| Ok(Box::new(BoolM::new(BoolOp::Intersect)))),
    ]
}

/// The closed boundary of a region-like entity.
fn entity_loop(e: &Entity, tol: f64) -> Option<Loop> {
    let lines = |pts: Vec<Vec2>| -> Loop {
        let n = pts.len();
        (0..n).filter_map(|i| Some(Segment::Line(Line::new(*pts.get(i)?, *pts.get((i + 1) % n)?)))).filter(|s| s.len() > 0.0).collect()
    };
    match &e.kind {
        EntityKind::LwPolyline(p) if p.closed && p.vertices.len() >= 2 => Some(Polyline { vertices: p.vertices.clone(), closed: true }.segments()),
        EntityKind::Circle(c) if c.radius > 0.0 => {
            let (ctr, r) = (c.center.xy(), c.radius);
            Some(vec![Segment::Arc { arc: Arc::new(ctr, r, 0.0, PI), ccw: true }, Segment::Arc { arc: Arc::new(ctr, r, PI, 0.0), ccw: true }])
        }
        EntityKind::Ellipse(el) => {
            let g = redrob_cad_geom::Ellipse { center: el.center.xy(), major: el.major.xy(), ratio: el.ratio, start: el.start, end: el.end };
            if !g.is_full() {
                return None;
            }
            let mut pts = Vec::new();
            g.tessellate(tol, &mut pts);
            pts.dedup_by(|a, b| a.near(*b, 1e-12));
            if pts.first().zip(pts.last()).is_some_and(|(a, b)| a.near(*b, 1e-9)) {
                pts.pop();
            }
            (pts.len() >= 3).then(|| lines(pts))
        }
        EntityKind::Spline(sp) if sp.closed => {
            let mut pts = sp.tessellate(tol);
            pts.dedup_by(|a, b| a.near(*b, 1e-12));
            if pts.first().zip(pts.last()).is_some_and(|(a, b)| a.near(*b, 1e-9)) {
                pts.pop();
            }
            (pts.len() >= 3).then(|| lines(pts))
        }
        _ => None,
    }
}

fn loop_vertices(l: &Loop) -> Vec<PolyVertex> {
    l.iter()
        .map(|s| PolyVertex {
            p: s.start(),
            bulge: match s {
                Segment::Line(_) => 0.0,
                Segment::Arc { arc, ccw } => arc_to_bulge(if *ccw { arc.sweep() } else { -arc.sweep() }),
            },
            ..Default::default()
        })
        .collect()
}

fn tolerance(s: &Session, hs: &[Handle]) -> f64 {
    let size = s
        .doc()
        .map(|d| {
            hs.iter()
                .filter_map(|h| d.entity(*h).map(|e| redrob_cad_doc::entity_bounds(d, e, 0)))
                .fold(redrob_cad_geom::Bounds2::EMPTY, |a, b| a.union(&b))
        })
        .map(|b| if b.is_empty() { 1.0 } else { b.width().max(b.height()) })
        .unwrap_or(1.0);
    (size * 1e-7).max(1e-9)
}

/// Loops of each handle, in order; objects that are not closed regions are reported.
fn loops_of(s: &Session, hs: &[Handle], tol: f64) -> Result<Vec<Loop>> {
    let d = s.doc()?;
    let mut out = Vec::new();
    for h in hs {
        let e = d.entity(*h).ok_or_else(|| EngineError::Other("no such object".into()))?;
        match entity_loop(e, tol) {
            Some(l) if !l.is_empty() => out.push(l),
            _ => return Err(EngineError::Other(format!("{} is not a region: use closed polylines, circles or REGION first.", e.kind.type_name()))),
        }
    }
    Ok(out)
}

/// Replace `inputs` by the result loops, keeping the first input's properties.
fn replace(s: &mut Session, inputs: &[Handle], result: Vec<Loop>) -> Result<Value> {
    let common =
        s.doc()?.entity(*inputs.first().ok_or_else(|| EngineError::Other("nothing selected".into()))?).map(|e| e.common.clone()).unwrap_or_default();
    let space = s.space();
    let d = s.doc_mut()?;
    for h in inputs {
        d.remove_entity(*h);
    }
    let mut out = Vec::new();
    for l in result {
        let nh = d.new_handle();
        if let Some(st) = d.space_mut(&space) {
            st.push(Entity { handle: nh, common: common.clone(), kind: lwpoly(loop_vertices(&l), true) });
        }
        out.push(nh.hex());
    }
    s.set_selection(Vec::new());
    let n = out.len();
    Ok(json!({ "handles": out, "message": if n == 0 { "Null region created - deleted.".to_string() } else { format!("{n} region(s)") } }))
}

fn fold(loops: Vec<Loop>, op: BoolOp, tol: f64) -> Vec<Loop> {
    let mut it = loops.into_iter();
    let Some(first) = it.next() else { return Vec::new() };
    let mut acc = vec![first];
    for l in it {
        acc = boolean(&acc, &[l], op, tol);
        if acc.is_empty() && op == BoolOp::Intersect {
            break;
        }
    }
    acc
}

fn run_bool(s: &mut Session, p: &Value, op: BoolOp) -> Result<Value> {
    let hs = targets(s, p)?;
    let id = if op == BoolOp::Union { "union" } else { "intersect" };
    if hs.len() < 2 {
        return Err(bad(id, if hs.is_empty() { "nothing selected".to_string() } else { "select at least two regions".to_string() }));
    }
    let tol = tolerance(s, &hs);
    let loops = loops_of(s, &hs, tol)?;
    let result = fold(loops, op, tol);
    replace(s, &hs, result)
}

fn handles(p: &Value, key: &str) -> Vec<Handle> {
    p.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().and_then(Handle::parse_hex)).collect()).unwrap_or_default()
}

fn run_subtract(s: &mut Session, p: &Value) -> Result<Value> {
    let from = handles(p, "from");
    let sub = handles(p, "subtract");
    if from.is_empty() {
        return Err(bad("subtract", "`from` is required"));
    }
    if sub.is_empty() {
        return Err(bad("subtract", "`subtract` is required"));
    }
    let all: Vec<Handle> = from.iter().chain(&sub).copied().collect();
    let tol = tolerance(s, &all);
    let a = fold(loops_of(s, &from, tol)?, BoolOp::Union, tol);
    let b = fold(loops_of(s, &sub, tol)?, BoolOp::Union, tol);
    let result = boolean(&a, &b, BoolOp::Subtract, tol);
    replace(s, &all, result)
}

/// UNION / INTERSECT: one selection. SUBTRACT: what to keep, then what to take away.
struct BoolM {
    op: BoolOp,
    sel: SelectPhase,
    from: Vec<Handle>,
    second: bool,
}

impl BoolM {
    fn new(op: BoolOp) -> Self {
        BoolM { op, sel: SelectPhase::default(), from: Vec::new(), second: false }
    }
    fn done(&mut self, s: &mut Session, hs: Vec<Handle>) -> Result<Step> {
        let r = match self.op {
            BoolOp::Subtract if !self.second => {
                self.from = hs;
                self.second = true;
                self.sel = SelectPhase::default();
                s.set_selection(Vec::new());
                return Ok(Step::Continue);
            }
            BoolOp::Subtract => run_subtract(
                s,
                &json!({ "from": self.from.iter().map(|h| h.hex()).collect::<Vec<_>>(), "subtract": hs.iter().map(|h| h.hex()).collect::<Vec<_>>() }),
            ),
            op => run_bool(s, &json!({ "handles": hs.iter().map(|h| h.hex()).collect::<Vec<_>>() }), op),
        };
        match r {
            Ok(v) => {
                if let Some(m) = v.get("message").and_then(Value::as_str) {
                    s.echo(m.to_string());
                }
            }
            Err(e) => s.echo(super::ask::plain(&e)),
        }
        Ok(Step::Done)
    }
}

impl Interactive for BoolM {
    fn name(&self) -> &'static str {
        match self.op {
            BoolOp::Union => "UNION",
            BoolOp::Subtract => "SUBTRACT",
            BoolOp::Intersect => "INTERSECT",
        }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        if self.op == BoolOp::Subtract {
            s.echo("Select solids, surfaces, and regions to subtract from ..");
        }
        self.sel = SelectPhase::begin(s);
        if self.sel.done {
            let hs = self.sel.picked.clone();
            return self.done(s, hs);
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
                if self.op == BoolOp::Subtract && !self.second {
                    s.echo("Select solids, surfaces, and regions to subtract ..");
                }
                self.done(s, hs)
            }
        }
    }
}
