//! Raster images (IMAGEATTACH): the picture itself under the linework. The engine stores only the
//! frame and the file path; pixels are loaded here once per path and kept as a texture. A file
//! that cannot be read shows the engine's empty frame, never an error.

use std::collections::HashMap;

use cadcraft_doc::EntityKind;
use cadcraft_geom::Vec2;
use egui::{Color32, Mesh, Pos2, TextureHandle, TextureOptions};

use crate::CadApp;
use crate::canvas::Xf;

/// Largest image side kept as a texture; bigger pictures are scaled down on load.
#[cfg(not(target_arch = "wasm32"))]
const MAX_SIDE: u32 = 4096;

#[derive(Default)]
pub struct Cache {
    /// `None` = the file was tried and could not be loaded (do not retry every frame).
    tex: HashMap<String, Option<TextureHandle>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn load(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 256 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE { img.thumbnail(MAX_SIDE, MAX_SIDE) } else { img };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(format!("raster:{path}"), color, TextureOptions::LINEAR))
}

#[cfg(target_arch = "wasm32")]
fn load(_ctx: &egui::Context, _path: &str) -> Option<TextureHandle> {
    None
}

/// Draw every raster image of the current space.
pub fn draw(app: &mut CadApp, ctx: &egui::Context, painter: &egui::Painter, xf: &Xf) {
    let space = app.session.space();
    let Ok(d) = app.session.doc() else { return };
    let Some(store) = d.space(&space) else { return };
    let mut quads: Vec<(String, [Vec2; 4])> = Vec::new();
    for e in store.iter() {
        if !d.is_visible(e) {
            continue;
        }
        if let EntityKind::Image(i) = &e.kind {
            if i.path.is_empty() {
                continue;
            }
            let o = i.insert.xy();
            let u = i.u.xy() * i.size.x;
            let v = i.v.xy() * i.size.y;
            quads.push((i.path.clone(), [o, o + u, o + u + v, o + v]));
        }
    }
    for (path, q) in quads {
        let tex = app.canvas.images.tex.entry(path.clone()).or_insert_with(|| load(ctx, &path));
        let Some(tex) = tex else { continue };
        let s: Vec<Pos2> = q.iter().map(|p| xf.to_screen(*p)).collect();
        let mut mesh = Mesh::with_texture(tex.id());
        // Image row 0 is the top: the frame's far edge (insert + v).
        let uv = [Pos2::new(0.0, 1.0), Pos2::new(1.0, 1.0), Pos2::new(1.0, 0.0), Pos2::new(0.0, 0.0)];
        for (p, t) in s.iter().zip(uv) {
            mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: t, color: Color32::WHITE });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }
}
