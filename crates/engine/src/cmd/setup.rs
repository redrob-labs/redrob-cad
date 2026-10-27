//! MVSETUP (model space): units, a scale factor and a paper size give a border the size of the
//! sheet at that scale, and the drawing limits follow it. A1–A4 and ISO scales are the usual input
//! (A3 at 1:50 -> a 21000 × 14850 border).

use serde_json::{Value, json};

use super::*;
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("mvsetup", "Drawing Setup", run_mvsetup)
            .menu(&["Format", "Drawing Setup"])
            .params("{units?: metric|decimal|engineering|architectural|scientific, scale, width, height, border?: bool (default true)}")
            .interactive(|_| Ok(Box::new(MvsetupM::default()))),
    ]
}

/// LUNITS and INSUNITS for a units keyword.
fn units_of(k: &str) -> Option<(i64, i64)> {
    Some(match k.trim().to_ascii_lowercase().as_str() {
        "s" | "scientific" => (1, 4),
        "d" | "decimal" => (2, 4),
        "e" | "engineering" => (3, 1),
        "a" | "architectural" => (4, 1),
        "m" | "metric" => (2, 4),
        _ => return None,
    })
}

fn run_mvsetup(s: &mut Session, p: &Value) -> Result<Value> {
    let scale = f64_req("mvsetup", p, "scale")?;
    let w = f64_req("mvsetup", p, "width")?;
    let h = f64_req("mvsetup", p, "height")?;
    if !(scale > 0.0 && w > 0.0 && h > 0.0) || scale * w.max(h) > 1e12 {
        return Err(bad("mvsetup", "scale, width and height must be positive"));
    }
    if let Some((lunits, insunits)) = str_param(p, "units").and_then(units_of) {
        let d = s.doc_mut()?;
        d.header.set_i64("LUNITS", lunits);
        d.header.set_i64("INSUNITS", insunits);
    }
    let (bw, bh) = (w * scale, h * scale);
    {
        let d = s.doc_mut()?;
        d.header.set("LIMMIN", redrob_cad_doc::HVal::Point(redrob_cad_geom::Vec3::new(0.0, 0.0, 0.0)));
        d.header.set("LIMMAX", redrob_cad_doc::HVal::Point(redrob_cad_geom::Vec3::new(bw, bh, 0.0)));
    }
    let mut border = Value::Null;
    if bool_or(p, "border", true) {
        let rect = find_command("rectang").ok_or_else(|| EngineError::UnknownCommand("rectang".into()))?;
        border = (rect.run)(s, &json!({ "p1": [0.0, 0.0], "p2": [bw, bh] }))?;
    }
    if let Some(z) = find_command("zoom") {
        let _ = (z.run)(s, &json!({ "mode": "extents" }));
    }
    Ok(json!({ "limits": [[0.0, 0.0], [bw, bh]], "border": border, "message": format!("Border {bw} x {bh} (scale {scale}).") }))
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Stage {
    #[default]
    PaperSpace,
    Units,
    Scale,
    Width,
    Height,
}

#[derive(Default)]
struct MvsetupM {
    stage: Stage,
    units: Option<String>,
    scale: f64,
    width: f64,
}

const METRIC_SCALES: &str = "Metric Scales\n================\n(5000) 1:5000\n(2000) 1:2000\n(1000) 1:1000\n(500) 1:500\n(200) 1:200\n(100) 1:100\n(75) 1:75\n(50) 1:50\n(20) 1:20\n(10) 1:10\n(5) 1:5\n(1) FULL";

impl Interactive for MvsetupM {
    fn name(&self) -> &'static str {
        "MVSETUP"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.stage {
            Stage::PaperSpace => Prompt::new("Enable paper space?", Accept::TEXT).kw(&["No", "Yes"]).default("Y"),
            Stage::Units => Prompt::new("Enter units type", Accept::TEXT).kw(&["Scientific", "Decimal", "Engineering", "Architectural", "Metric"]),
            Stage::Scale => Prompt::new("Enter the scale factor", Accept::NUMBER),
            Stage::Width => Prompt::new("Enter the paper width", Accept::NUMBER),
            Stage::Height => Prompt::new("Enter the paper height", Accept::NUMBER),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let t = match i {
            Input::Text(t) | Input::Keyword(t) => t.trim().to_string(),
            Input::Enter => String::new(),
            Input::Cancel => return Ok(Step::Cancel),
            _ => return Ok(Step::Continue),
        };
        let num = |t: &str| crate::units::parse_distance(t).filter(|f| f.is_finite() && *f > 0.0);
        match self.stage {
            Stage::PaperSpace => {
                if t.is_empty() || t.to_ascii_lowercase().starts_with('y') {
                    s.echo("Paper space setup is done on a layout tab: switch to a layout and use its Page Setup.");
                    return Ok(Step::Done);
                }
                if !t.to_ascii_lowercase().starts_with('n') {
                    s.echo("Invalid option keyword.");
                    return Ok(Step::Continue);
                }
                self.stage = Stage::Units;
            }
            Stage::Units => {
                if units_of(&t).is_none() {
                    s.echo("Invalid option keyword.");
                    return Ok(Step::Continue);
                }
                if t.to_ascii_lowercase().starts_with('m') {
                    s.echo(METRIC_SCALES);
                }
                self.units = Some(t);
                self.stage = Stage::Scale;
            }
            Stage::Scale => match num(&t) {
                Some(f) => {
                    self.scale = f;
                    self.stage = Stage::Width;
                }
                None => s.echo("Requires a positive number."),
            },
            Stage::Width => match num(&t) {
                Some(f) => {
                    self.width = f;
                    self.stage = Stage::Height;
                }
                None => s.echo("Requires a positive number."),
            },
            Stage::Height => match num(&t) {
                Some(h) => {
                    let r = run_mvsetup(s, &json!({ "units": self.units, "scale": self.scale, "width": self.width, "height": h }))?;
                    if let Some(m) = r.get("message").and_then(Value::as_str) {
                        s.echo(m.to_string());
                    }
                    return Ok(Step::Done);
                }
                None => s.echo("Requires a positive number."),
            },
        }
        Ok(Step::Continue)
    }
}
