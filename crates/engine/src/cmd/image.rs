//! IMAGEATTACH: place a raster image (PNG, JPEG, GIF, BMP) as an IMAGE entity. The drawing keeps
//! the file path and the frame; the UI paints the pixels. ATTACH picks image or drawing reference
//! by the file's extension.

use redrob_cad_doc::{EntityKind, Image};
use redrob_cad_geom::{Vec2, Vec3};
use serde_json::{Value, json};

use super::*;
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("imageattach", "Raster Image Reference...", run_imageattach)
            .menu(&["Insert", "Raster Image Reference..."])
            .alias(&["iat", "image", "im"])
            .params("{path, at: [x,y], scale? (frame width in drawing units, default 1), rotation? (degrees)}")
            .interactive(|_| Ok(Box::new(AttachM::new(true)))),
        CommandSpec::new("attach", "Attach...", |s, p| {
            let path = str_param(p, "path").ok_or_else(|| bad("attach", "`path` is required"))?;
            let id = if is_image(path) { "imageattach" } else { "xattach" };
            let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.into()))?;
            (spec.run)(s, p)
        })
        .menu(&["File", "Attach..."])
        .params("{path, at: [x,y], scale?, rotation?} (an image file attaches as an image, a drawing as a reference)")
        .interactive(|_| Ok(Box::new(AttachM::new(false)))),
    ]
}

fn is_image(path: &str) -> bool {
    let l = path.trim().to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".bmp"].iter().any(|e| l.ends_with(e))
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}
fn le32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}
fn be16(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?])))
}
fn le16(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?])))
}

/// Pixel width and height from an image file's header (no decoding).
pub(crate) fn pixel_size(b: &[u8]) -> Option<(u32, u32)> {
    let wh = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        (be32(b, 16)?, be32(b, 20)?)
    } else if b.starts_with(b"GIF8") {
        (le16(b, 6)?, le16(b, 8)?)
    } else if b.starts_with(b"BM") {
        (le32(b, 18)?, (le32(b, 22)? as i32).unsigned_abs())
    } else if b.starts_with(&[0xff, 0xd8]) {
        // JPEG: walk the segments to the first start-of-frame marker.
        let mut i = 2usize;
        let mut found = None;
        for _ in 0..4096 {
            if *b.get(i)? != 0xff {
                return None;
            }
            let m = *b.get(i + 1)?;
            if (0xc0..=0xcf).contains(&m) && !matches!(m, 0xc4 | 0xc8 | 0xcc) {
                found = Some((be16(b, i + 7)?, be16(b, i + 5)?));
                break;
            }
            i = i.checked_add(2 + be16(b, i + 2)? as usize)?;
        }
        found?
    } else {
        return None;
    };
    (wh.0 > 0 && wh.1 > 0 && wh.0 <= 1 << 20 && wh.1 <= 1 << 20).then_some(wh)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_size(path: &str) -> Result<(u32, u32)> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| bad("imageattach", format!("{path}: {e}")))?;
    // Headers sit near the start; JPEG metadata (EXIF) can push the frame marker out a little.
    let mut buf = Vec::new();
    f.by_ref().take(1 << 20).read_to_end(&mut buf).map_err(|e| bad("imageattach", format!("{path}: {e}")))?;
    pixel_size(&buf).ok_or_else(|| bad("imageattach", format!("{path}: not a PNG, JPEG, GIF or BMP image")))
}

#[cfg(target_arch = "wasm32")]
fn read_size(_path: &str) -> Result<(u32, u32)> {
    Err(bad("imageattach", "files are not available on the web"))
}

fn run_imageattach(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("imageattach", "`path` is required"))?.trim().trim_matches('"').to_string();
    let at = point_req("imageattach", p, "at")?;
    let width = f64_or(p, "scale", 1.0);
    if width.is_nan() || width <= 0.0 || width > 1e12 {
        return Err(bad("imageattach", "scale must be positive"));
    }
    let rot = f64_or(p, "rotation", 0.0).to_radians();
    let (pw, ph) = read_size(&path)?;
    // One pixel in drawing units, along the image's own x and y.
    let px = width / f64::from(pw);
    let (c, sn) = (rot.cos(), rot.sin());
    let h = s.add_entity(EntityKind::Image(Image {
        insert: Vec3::new(at.x, at.y, 0.0),
        u: Vec3::new(c * px, sn * px, 0.0),
        v: Vec3::new(-sn * px, c * px, 0.0),
        size: Vec2::new(f64::from(pw), f64::from(ph)),
        path: path.clone(),
    }))?;
    Ok(json!({ "handle": h.hex(), "pixels": [pw, ph], "size": [width, width * f64::from(ph) / f64::from(pw)] }))
}

/// IMAGEATTACH / ATTACH at the command line: file, insertion point, scale.
struct AttachM {
    image_only: bool,
    path: Option<String>,
    at: Option<Vec2>,
}

impl AttachM {
    fn new(image_only: bool) -> Self {
        AttachM { image_only, path: None, at: None }
    }
}

impl Interactive for AttachM {
    fn name(&self) -> &'static str {
        if self.image_only { "IMAGEATTACH" } else { "ATTACH" }
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match (&self.path, self.at) {
            (None, _) => Prompt::new(if self.image_only { "Enter image file name" } else { "Enter name of file to attach" }, Accept::TEXT),
            (Some(_), None) => Prompt::new("Specify insertion point", Accept::POINT),
            (Some(_), Some(a)) => Prompt::new("Specify scale factor", Accept::POINT_OR_NUMBER).default("1").base(a),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (&self.path, self.at, i) {
            (None, _, Input::Text(t) | Input::Keyword(t)) if !t.trim().is_empty() => {
                let t = t.trim().trim_matches('"').to_string();
                if self.image_only && !is_image(&t) {
                    s.echo("Not a PNG, JPEG, GIF or BMP image.");
                    return Ok(Step::Continue);
                }
                self.path = Some(t);
                Ok(Step::Continue)
            }
            (Some(_), None, Input::Point(p)) => {
                self.at = Some(p);
                Ok(Step::Continue)
            }
            (Some(path), Some(a), i) => {
                let scale = match i {
                    Input::Text(t) => {
                        super::machines::number(&t).filter(|f| *f > 0.0).ok_or_else(|| EngineError::Other("Requires a positive number.".into()))?
                    }
                    Input::Point(p) => a.dist(p).max(1e-9),
                    Input::Cancel => return Ok(Step::Cancel),
                    _ => 1.0,
                };
                let id = if is_image(path) { "imageattach" } else { "xattach" };
                let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.into()))?;
                (spec.run)(s, &json!({ "path": path, "at": [a.x, a.y], "scale": scale }))?;
                Ok(Step::Done)
            }
            (_, _, Input::Cancel | Input::Enter) => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
}
