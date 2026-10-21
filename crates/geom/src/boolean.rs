//! Boolean operations on planar regions bounded by lines and arcs: union, subtract, intersect
//! (AutoCAD UNION / SUBTRACT / INTERSECT on regions).
//!
//! Every boundary segment of one operand is split where it crosses the other operand's
//! boundary. Each piece is then inside, outside or on the other region, judged at its midpoint;
//! the operation keeps a fixed choice of pieces and the kept pieces are chained back into closed
//! loops. Arcs stay arcs. Holes come back as separate loops running the other way.

use crate::{Arc, Line, Segment, Vec2, intersect_ext, norm_angle, point_in_polygon, shoelace};

/// Upper bound on boundary segments per operand (the splitting is quadratic).
pub const MAX_SEGMENTS: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

/// A closed boundary: consecutive segments, the last ending where the first starts.
pub type Loop = Vec<Segment>;

/// Fraction (0..1, in segment direction) of `p` along `s`.
fn param(s: &Segment, p: Vec2) -> f64 {
    match s {
        Segment::Line(l) => l.param_of(p).clamp(0.0, 1.0),
        Segment::Arc { arc, ccw } => {
            let sw = arc.sweep();
            if sw <= 0.0 {
                return 0.0;
            }
            let f = (norm_angle(arc.center.angle_to(p) - arc.start) / sw).clamp(0.0, 1.0);
            if *ccw { f } else { 1.0 - f }
        }
    }
}

/// The piece of `s` between fractions `t0 < t1`.
fn piece(s: &Segment, t0: f64, t1: f64) -> Segment {
    let (p0, p1) = (s.at(t0), s.at(t1));
    match s {
        Segment::Line(_) => Segment::Line(Line::new(p0, p1)),
        Segment::Arc { arc, ccw } => {
            let (a0, a1) = (arc.center.angle_to(p0), arc.center.angle_to(p1));
            if *ccw {
                Segment::Arc { arc: Arc::new(arc.center, arc.radius, a0, a1), ccw: true }
            } else {
                Segment::Arc { arc: Arc::new(arc.center, arc.radius, a1, a0), ccw: false }
            }
        }
    }
}

fn tessellate(loops: &[Loop], tol: f64) -> Vec<Vec<Vec2>> {
    loops
        .iter()
        .map(|l| {
            let mut pts = Vec::new();
            for s in l {
                s.tessellate(tol, &mut pts);
            }
            pts.dedup_by(|a, b| a.near(*b, 1e-12));
            pts
        })
        .collect()
}

/// Even-odd inside test against a set of loops.
fn inside(polys: &[Vec<Vec2>], p: Vec2) -> bool {
    polys.iter().filter(|poly| point_in_polygon(poly, p)).count() % 2 == 1
}

/// Signed area of a loop (positive = counter-clockwise).
pub fn loop_area(l: &Loop, tol: f64) -> f64 {
    let mut pts = Vec::new();
    for s in l {
        s.tessellate(tol, &mut pts);
    }
    shoelace(&pts)
}

/// The loop running the other way.
pub fn reversed(l: &Loop) -> Loop {
    l.iter().rev().map(Segment::reversed).collect()
}

/// Split every segment of `a` at its crossings with `b`.
fn split(a: &[Loop], b: &[Loop], tol: f64) -> Vec<Segment> {
    let bsegs: Vec<&Segment> = b.iter().flatten().collect();
    let mut out = Vec::new();
    for s in a.iter().flatten() {
        let sb = s.bounds().expand(tol);
        let mut ts: Vec<f64> = vec![0.0, 1.0];
        for o in &bsegs {
            if !sb.intersects(&o.bounds().expand(tol)) {
                continue;
            }
            for x in intersect_ext(s, o, false) {
                ts.push(param(s, x));
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|x, y| (*x - *y).abs() * s.len().max(1e-300) < tol);
        for w in ts.windows(2) {
            if let [t0, t1] = w
                && (t1 - t0) * s.len() > tol
            {
                out.push(piece(s, *t0, *t1));
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Where {
    In,
    Out,
    /// On the other boundary, running the same way (true) or the opposite way.
    On(bool),
}

fn classify(p: &Segment, other: &[Loop], polys: &[Vec<Vec2>], tol: f64) -> Where {
    let m = p.mid();
    for s in other.iter().flatten() {
        if s.closest(m).dist(m) < tol * 4.0 {
            let t = param(s, m);
            let same = s.tangent(t).dot(p.tangent(0.5)) > 0.0;
            return Where::On(same);
        }
    }
    if inside(polys, m) { Where::In } else { Where::Out }
}

/// Chain pieces end to start into closed loops; open chains are dropped.
fn chain(mut pieces: Vec<Segment>, tol: f64) -> Vec<Loop> {
    let join = tol * 10.0;
    let mut loops = Vec::new();
    let mut guard = 0usize;
    while let Some(first) = pieces.pop() {
        let mut cur = vec![first];
        loop {
            guard += 1;
            if guard > 4 * MAX_SEGMENTS {
                return loops;
            }
            let (Some(start), Some(end)) = (cur.first().map(Segment::start), cur.last().map(Segment::end)) else { break };
            if cur.len() > 1 && end.near(start, join) {
                loops.push(cur);
                break;
            }
            match pieces.iter().position(|s| s.start().near(end, join)) {
                Some(i) => cur.push(pieces.swap_remove(i)),
                None => break,
            }
        }
    }
    loops
}

/// `a op b`. Every input loop is taken counter-clockwise; overlapping loops of one operand
/// combine even-odd (a loop inside another of the same operand is a hole).
pub fn boolean(a: &[Loop], b: &[Loop], op: BoolOp, tol: f64) -> Vec<Loop> {
    let tol = if tol.is_finite() && tol > 0.0 { tol } else { 1e-6 };
    if a.iter().map(Vec::len).sum::<usize>() > MAX_SEGMENTS || b.iter().map(Vec::len).sum::<usize>() > MAX_SEGMENTS {
        return Vec::new();
    }
    let ccw = |ls: &[Loop]| -> Vec<Loop> { ls.iter().map(|l| if loop_area(l, tol) < 0.0 { reversed(l) } else { l.clone() }).collect() };
    let (a, b) = (ccw(a), ccw(b));
    let ttol = tol * 10.0;
    let (pa, pb) = (tessellate(&a, ttol), tessellate(&b, ttol));
    let mut keep = Vec::new();
    for p in split(&a, &b, tol) {
        let w = classify(&p, &b, &pb, tol);
        let k = match op {
            BoolOp::Union => matches!(w, Where::Out | Where::On(true)),
            BoolOp::Intersect => matches!(w, Where::In | Where::On(true)),
            BoolOp::Subtract => matches!(w, Where::Out | Where::On(false)),
        };
        if k {
            keep.push(p);
        }
    }
    for p in split(&b, &a, tol) {
        // Pieces of b on a's boundary were already taken from a (or are cancelled).
        match (op, classify(&p, &a, &pa, tol)) {
            (BoolOp::Union, Where::Out) | (BoolOp::Intersect, Where::In) => keep.push(p),
            (BoolOp::Subtract, Where::In) => keep.push(p.reversed()),
            _ => {}
        }
    }
    chain(keep, tol).into_iter().filter(|l| loop_area(l, ttol).abs() > tol * tol).collect()
}
