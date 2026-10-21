//! MLEDIT: wall-joint edits on multilines. A multiline here is its parallel polylines (MLINE
//! draws one per element), so a pick finds the picked polyline and the parallel polylines that
//! run beside it. Tools: Closed/Open/Merged Cross, Closed/Open/Merged Tee, Corner Joint,
//! Cut Single, Cut All.

use cadcraft_doc::{EntityKind, Handle, LwPolyline};
use cadcraft_geom::{Vec2, line_line_infinite};
use serde_json::{Value, json};

use super::*;
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("mledit", "Multiline...", run_mledit)
            .menu(&["Modify", "Object", "Multiline..."])
            .alias(&["-mledit"])
            .params("{tool: cc|oc|mc|ct|ot|mt|cj|cs|ca, p1: [x,y] (on the first multiline), p2: [x,y] (second multiline, or second cut point)}")
            .interactive(|_| Ok(Box::new(MleditM::default()))),
    ]
}

const TOOLS: &[&str] = &["CC", "OC", "MC", "CT", "OT", "MT", "CJ", "CS", "CA"];

type Seg = (Vec2, Vec2);

fn poly(s: &Session, h: Handle) -> Option<LwPolyline> {
    match &s.doc().ok()?.entity(h)?.kind {
        EntityKind::LwPolyline(p) => Some(p.clone()),
        _ => None,
    }
}

/// Straight segments of a polyline (arcs are skipped; MLINE draws straight walls).
fn segs(p: &LwPolyline) -> Vec<Seg> {
    let n = p.vertices.len();
    let count = if p.closed { n } else { n.saturating_sub(1) };
    (0..count)
        .filter_map(|i| {
            let a = p.vertices.get(i)?;
            let b = p.vertices.get((i + 1) % n.max(1))?;
            (a.bulge.abs() < 1e-12).then_some((a.p, b.p))
        })
        .collect()
}

fn dist_to_seg(p: Vec2, (a, b): Seg) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    if l2 < 1e-24 {
        return p.dist(a);
    }
    let t = ((p - a).dot(d) / l2).clamp(0.0, 1.0);
    p.dist(a + d * t)
}

/// Polylines running parallel beside `h` at a constant spacing: the other elements of its multiline.
fn partners(s: &Session, h: Handle) -> Result<Vec<Handle>> {
    let me = poly(s, h).ok_or_else(|| EngineError::Other("Select a multiline.".into()))?;
    let ms = segs(&me);
    let d = s.doc()?;
    let space = s.space();
    let store = d.space(&space).ok_or(EngineError::NoDocument)?;
    let mut out = Vec::new();
    for e in store.iter().take(200_000) {
        if e.handle == h || !d.is_visible(e) {
            continue;
        }
        let EntityKind::LwPolyline(o) = &e.kind else { continue };
        if o.closed != me.closed || o.vertices.len() != me.vertices.len() {
            continue;
        }
        let os = segs(o);
        if os.len() != ms.len() || os.is_empty() {
            continue;
        }
        // Each segment parallel to its twin, all at the same spacing.
        let mut gap = None;
        let ok = ms.iter().zip(&os).all(|(&(a, b), &(c, dd))| {
            let u = b - a;
            let v = dd - c;
            let (lu, lv) = (u.len(), v.len());
            if lu < 1e-12 || lv < 1e-12 || (u.cross(v) / (lu * lv)).abs() > 1e-6 || u.dot(v) < 0.0 {
                return false;
            }
            let g = (c - a).cross(u).abs() / lu;
            match gap {
                None => {
                    gap = Some(g);
                    g > 1e-9
                }
                Some(g0) => (g - g0).abs() <= 1e-6 * g0.max(1.0),
            }
        });
        if ok && let Some(g) = gap {
            out.push((e.handle, g));
        }
    }
    // Elements of one multiline sit within its width; keep the ones spaced like the nearest.
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    let Some(&(_, nearest)) = out.first() else {
        return Err(EngineError::Other("That object is not a multiline.".into()));
    };
    Ok(out.into_iter().filter(|(_, g)| *g <= nearest * 8.0 + 1e-9).map(|(h, _)| h).collect())
}

/// The multiline (its element polylines) under `p`.
fn mline_at(s: &Session, p: Vec2) -> Result<Vec<Handle>> {
    let ap = s.pixel_size() * s.settings.pickbox.max(1.0) * 1.5;
    let space = s.space();
    let h = crate::select::pick(s.doc()?, &space, p, ap).ok_or_else(|| EngineError::Other("Select a multiline.".into()))?;
    let mut v = vec![h];
    v.extend(partners(s, h)?);
    Ok(v)
}

/// Bounded crossings of polyline `a` with polyline `b`.
fn crossings(a: &LwPolyline, b: &LwPolyline) -> Vec<Vec2> {
    let mut out = Vec::new();
    for &(a1, a2) in &segs(a) {
        for &(b1, b2) in &segs(b) {
            if let Some((x, t, u)) = line_line_infinite(a1, a2, b1, b2)
                && (-1e-9..=1.0 + 1e-9).contains(&t)
                && (-1e-9..=1.0 + 1e-9).contains(&u)
            {
                out.push(x);
            }
        }
    }
    out
}

/// Which end of open polyline `a` to move, and where, so it ends on polyline `b`: the end
/// segment's line extended or cut back to `b`. The end nearest its meeting point wins, unless
/// moving it would cut away `keep` (the point the user picked on the part to keep).
fn end_hit(a: &LwPolyline, b: &LwPolyline, keep: Vec2, bounded: bool) -> Option<(bool, Vec2)> {
    if a.closed {
        return None;
    }
    let ss = segs(a);
    let first = *ss.first()?;
    let last = *ss.last()?;
    let mut best: Option<(bool, Vec2, f64)> = None;
    // (moves start?, the fixed point of the end segment, the end vertex)
    for (at_start, fixed, end) in [(true, first.1, first.0), (false, last.0, last.1)] {
        for &(b1, b2) in &segs(b) {
            let Some((x, _t, u)) = line_line_infinite(fixed, end, b1, b2) else { continue };
            if bounded && !(-1e-9..=1.0 + 1e-9).contains(&u) {
                continue;
            }
            // keep strictly between the end and x would be cut away.
            let cuts_keep =
                (x - end).dot(keep - end) > 0.0 && (keep - end).len() < (x - end).len() && dist_to_seg(keep, (end, x)) < 1e-6 * (1.0 + x.dist(end));
            if cuts_keep {
                continue;
            }
            let d = end.dist(x);
            if best.as_ref().is_none_or(|bb| d < bb.2) {
                best = Some((at_start, x, d));
            }
        }
    }
    best.map(|(s, x, _)| (s, x))
}

fn move_end(s: &mut Session, h: Handle, at_start: bool, p: Vec2) -> Result<()> {
    s.doc_mut()?.modify_entity(h, |e| {
        if let EntityKind::LwPolyline(pl) = &mut e.kind {
            let i = if at_start { 0 } else { pl.vertices.len().saturating_sub(1) };
            if let Some(v) = pl.vertices.get_mut(i) {
                v.p = p;
            }
        }
    })?;
    Ok(())
}

/// Cut a gap in `h` between the two crossings nearest each other along it.
fn gap(s: &mut Session, h: Handle, pts: &[Vec2]) -> Result<()> {
    let (Some(&f), Some(&l)) = (pts.first(), pts.last()) else { return Ok(()) };
    if pts.len() < 2 {
        return Ok(());
    }
    let (mut x1, mut x2) = (f, l);
    // The widest pair: the outer elements of the crossing multiline.
    let mut best = 0.0;
    for (i, a) in pts.iter().enumerate() {
        for b in pts.iter().skip(i + 1) {
            if a.dist(*b) > best {
                best = a.dist(*b);
                (x1, x2) = (*a, *b);
            }
        }
    }
    super::modify::break_entity(s, h, x1, x2)?;
    Ok(())
}

fn outer_crossings(s: &Session, a: Handle, b: &[Handle]) -> Vec<Vec2> {
    let Some(pa) = poly(s, a) else { return Vec::new() };
    b.iter().filter_map(|h| poly(s, *h)).flat_map(|pb| crossings(&pa, &pb)).collect()
}

/// Make the elements of stem `a` end at the nearest element of bar `b` (closed tee).
fn tee(s: &mut Session, a: &[Handle], b: &[Handle], keep: Vec2) -> Result<Vec<Vec2>> {
    // The bar element facing the stem: nearest to the kept point.
    let near = b
        .iter()
        .filter_map(|h| poly(s, *h).map(|p| (*h, segs(&p).iter().map(|sg| dist_to_seg(keep, *sg)).fold(f64::INFINITY, f64::min))))
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|x| x.0)
        .ok_or_else(|| EngineError::Other("Select a multiline.".into()))?;
    let bar = poly(s, near).ok_or_else(|| EngineError::Other("Select a multiline.".into()))?;
    let mut ends = Vec::new();
    for h in a {
        let Some(pa) = poly(s, *h) else { continue };
        if let Some((at_start, x)) = end_hit(&pa, &bar, keep, true) {
            move_end(s, *h, at_start, x)?;
            ends.push(x);
        }
    }
    if ends.is_empty() {
        return Err(EngineError::Other("The multilines do not meet.".into()));
    }
    Ok(ends)
}

fn corner(s: &mut Session, a: &[Handle], b: &[Handle], pa_keep: Vec2, pb_keep: Vec2) -> Result<()> {
    // Pair inner with inner and outer with outer, then make each pair meet.
    let pa: Vec<(Handle, LwPolyline)> = a.iter().filter_map(|h| poly(s, *h).map(|p| (*h, p))).collect();
    let pb: Vec<(Handle, LwPolyline)> = b.iter().filter_map(|h| poly(s, *h).map(|p| (*h, p))).collect();
    if pa.is_empty() || pb.is_empty() {
        return Err(EngineError::Other("Select a multiline.".into()));
    }
    // Order both by distance from the other multiline's kept point: inner first.
    let rank = |set: &[(Handle, LwPolyline)], from: Vec2| {
        let mut v: Vec<(Handle, LwPolyline, f64)> =
            set.iter().map(|(h, p)| (*h, p.clone(), segs(p).iter().map(|sg| dist_to_seg(from, *sg)).fold(f64::INFINITY, f64::min))).collect();
        v.sort_by(|x, y| x.2.total_cmp(&y.2));
        v
    };
    let ra = rank(&pa, pb_keep);
    let rb = rank(&pb, pa_keep);
    let mut done = 0;
    for ((ha, la, _), (hb, lb, _)) in ra.iter().zip(rb.iter()) {
        let ea = end_hit(la, lb, pa_keep, false);
        let eb = end_hit(lb, la, pb_keep, false);
        if let (Some((sa, xa)), Some((sb, _))) = (ea, eb) {
            move_end(s, *ha, sa, xa)?;
            move_end(s, *hb, sb, xa)?;
            done += 1;
        }
    }
    if done == 0 {
        return Err(EngineError::Other("The multilines do not meet.".into()));
    }
    Ok(())
}

pub(crate) fn apply(s: &mut Session, tool: &str, p1: Vec2, p2: Vec2) -> Result<Value> {
    let tool = tool.trim().to_ascii_uppercase();
    match tool.as_str() {
        "CS" | "CA" => {
            let a = mline_at(s, p1)?;
            let hs: Vec<Handle> = if tool == "CS" { a.into_iter().take(1).collect() } else { a };
            for h in &hs {
                super::modify::break_entity(s, *h, p1, p2)?;
            }
            return Ok(json!({ "tool": tool, "cut": hs.len() }));
        }
        _ => {}
    }
    let a = mline_at(s, p1)?;
    let b = mline_at(s, p2)?;
    if a.iter().any(|h| b.contains(h)) {
        return Err(EngineError::Other("Select two different multilines.".into()));
    }
    match tool.as_str() {
        "CC" | "OC" | "MC" => {
            let ca: Vec<(Handle, Vec<Vec2>)> = a.iter().map(|h| (*h, outer_crossings(s, *h, &b))).collect();
            let cb: Vec<(Handle, Vec<Vec2>)> = b.iter().map(|h| (*h, outer_crossings(s, *h, &a))).collect();
            if ca.iter().all(|(_, v)| v.is_empty()) {
                return Err(EngineError::Other("The multilines do not cross.".into()));
            }
            for (h, pts) in &ca {
                gap(s, *h, pts)?;
            }
            if tool != "CC" {
                for (h, pts) in &cb {
                    gap(s, *h, pts)?;
                }
            }
        }
        "CT" | "OT" | "MT" => {
            let ends = tee(s, &a, &b, p1)?;
            if tool != "CT" {
                // Open the bar between the stem's outer elements.
                let near = b
                    .iter()
                    .filter_map(|h| poly(s, *h).map(|p| (*h, segs(&p).iter().map(|sg| dist_to_seg(p1, *sg)).fold(f64::INFINITY, f64::min))))
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                    .map(|x| x.0);
                if let Some(n) = near {
                    gap(s, n, &ends)?;
                }
            }
        }
        "CJ" => corner(s, &a, &b, p1, p2)?,
        _ => return Err(bad("mledit", format!("unknown tool `{tool}` (use {})", TOOLS.join(", ")))),
    }
    Ok(json!({ "tool": tool }))
}

fn run_mledit(s: &mut Session, p: &Value) -> Result<Value> {
    let tool = str_param(p, "tool").ok_or_else(|| bad("mledit", "`tool` is required"))?.to_string();
    let p1 = point_req("mledit", p, "p1")?;
    let p2 = point_req("mledit", p, "p2")?;
    apply(s, &tool, p1, p2)
}

#[derive(Default)]
struct MleditM {
    tool: Option<String>,
    p1: Option<Vec2>,
}

impl Interactive for MleditM {
    fn name(&self) -> &'static str {
        "MLEDIT"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        let cut = self.tool.as_deref().is_some_and(|t| t == "CS" || t == "CA");
        match (&self.tool, self.p1) {
            (None, _) => Prompt::new("Enter mline editing option", Accept::TEXT).kw(TOOLS),
            (Some(_), None) => Prompt::new(if cut { "Select mline" } else { "Select first mline" }, Accept::POINT),
            (Some(_), Some(p)) => Prompt::new(if cut { "Select second point" } else { "Select second mline" }, Accept::POINT).base(p),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (&self.tool, self.p1, i) {
            (None, _, Input::Text(t) | Input::Keyword(t)) => {
                let t = t.trim().to_ascii_uppercase();
                if TOOLS.contains(&t.as_str()) {
                    self.tool = Some(t);
                } else {
                    s.echo("Invalid option keyword.");
                }
                Ok(Step::Continue)
            }
            (Some(_), None, Input::Point(p)) => {
                self.p1 = Some(p);
                Ok(Step::Continue)
            }
            (Some(tool), Some(p1), Input::Point(p2)) => {
                let tool = tool.clone();
                match apply(s, &tool, p1, p2) {
                    Ok(_) => {
                        // AutoCAD keeps asking for the next pair with the same tool.
                        self.p1 = None;
                        Ok(Step::Continue)
                    }
                    Err(e) => {
                        s.echo(super::ask::plain(&e));
                        self.p1 = None;
                        Ok(Step::Continue)
                    }
                }
            }
            (_, _, Input::Enter | Input::Cancel) => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
}
