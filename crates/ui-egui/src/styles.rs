//! Plot, Text Style and Dimension Style dialogs. PLOT (Ctrl+P), STYLE (ST) and DIMSTYLE (D)
//! typed with nothing open these; JSON calls still run the commands directly.
//! Every change goes through a command (`plot`, `style`, `dimstyle`), so the dialogs stay thin.

use redrob_cad_geom::Vec2;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::CadApp;
use crate::i18n::tr;

/// Plot dialog settings, kept between plots.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlotSettings {
    /// A paper name from `redrob_cad_render::PAPER_SIZES`.
    pub paper: String,
    pub landscape: bool,
    /// Zero margins (ISO full bleed).
    pub full_bleed: bool,
    /// "extents" | "display" | "limits" | "window".
    pub area: String,
    pub fit: bool,
    /// Plot scale as paper units = drawing units.
    pub paper_units: f64,
    pub drawing_units: f64,
    pub center: bool,
    pub lineweights: bool,
    /// Reopen the dialog when the "Window<" pick finishes.
    pub picking: bool,
}

impl Default for PlotSettings {
    fn default() -> Self {
        PlotSettings {
            paper: "ISO A4 (210.00 x 297.00 MM)".into(),
            landscape: true,
            full_bleed: false,
            area: "extents".into(),
            fit: true,
            paper_units: 1.0,
            drawing_units: 1.0,
            center: true,
            lineweights: true,
            picking: false,
        }
    }
}

const AREAS: &[(&str, &str)] = &[("extents", "Extents"), ("display", "Display"), ("limits", "Limits"), ("window", "Window")];

/// Called every frame: brings the Plot dialog back after "Window<".
pub fn tick(app: &mut CadApp) {
    if app.ui.plot.picking && app.session.running.is_none() {
        app.ui.plot.picking = false;
        if app.session.plot_window.is_some() {
            app.ui.plot.area = "window".into();
        }
        app.ui.dialog = Some("plot".into());
    }
}

fn header_pt(app: &CadApp, k: &str) -> Option<Vec2> {
    match app.session.doc().ok()?.header.get(k)? {
        redrob_cad_doc::HVal::Point(p) => Some(p.xy()),
        _ => None,
    }
}

/// The plot area as a window, or `None` for extents.
fn area_window(app: &CadApp) -> Option<(Vec2, Vec2)> {
    match app.ui.plot.area.as_str() {
        "window" => app.session.plot_window,
        "limits" => Some((header_pt(app, "LIMMIN")?, header_pt(app, "LIMMAX")?)),
        "display" => app.canvas.xf.map(|x| {
            let b = x.world_bounds();
            (b.min, b.max)
        }),
        _ => None,
    }
}

fn plot_params(app: &CadApp) -> Value {
    let p = &app.ui.plot;
    let mut v = json!({
        "paper": p.paper,
        "landscape": p.landscape,
        "fit": p.fit,
        "center": p.center,
        "lineweights": p.lineweights,
    });
    if let Some(o) = v.as_object_mut() {
        if !p.fit && p.paper_units > 0.0 && p.drawing_units > 0.0 {
            o.insert("scale".into(), json!(p.paper_units / p.drawing_units));
        }
        if p.full_bleed {
            o.insert("margins".into(), json!([0.0, 0.0, 0.0, 0.0]));
        }
        if let Some((a, b)) = area_window(app) {
            o.insert("window".into(), json!([[a.x, a.y], [b.x, b.y]]));
        }
    }
    v
}

pub fn plot(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let mut go = false;
    let mut pick = false;
    egui::Window::new(tr("Plot")).open(open).resizable(false).collapsible(false).show(ctx, |ui| {
        let p = &mut app.ui.plot;
        egui::Grid::new("plot_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tr("Printer/plotter"));
            ui.label(tr("PDF"));
            ui.end_row();
            ui.label(tr("Paper size"));
            egui::ComboBox::from_id_salt("plot_paper").selected_text(p.paper.clone()).width(260.0).show_ui(ui, |ui| {
                for ps in redrob_cad_render::PAPER_SIZES {
                    ui.selectable_value(&mut p.paper, ps.name.to_string(), ps.name);
                }
            });
            ui.end_row();
            ui.label("");
            ui.horizontal(|ui| {
                ui.checkbox(&mut p.landscape, tr("Landscape"));
                ui.checkbox(&mut p.full_bleed, tr("Full bleed (no margins)"));
            });
            ui.end_row();
            ui.label(tr("What to plot"));
            ui.horizontal(|ui| {
                let cur = AREAS.iter().find(|a| a.0 == p.area).map(|a| a.1).unwrap_or("Extents");
                egui::ComboBox::from_id_salt("plot_area").selected_text(tr(cur)).show_ui(ui, |ui| {
                    for (k, l) in AREAS {
                        ui.selectable_value(&mut p.area, (*k).to_string(), tr(l));
                    }
                });
                if p.area == "window" && ui.button(tr("Window<")).clicked() {
                    pick = true;
                }
            });
            ui.end_row();
            ui.label(tr("Plot scale"));
            ui.vertical(|ui| {
                ui.checkbox(&mut p.fit, tr("Fit to paper"));
                ui.add_enabled_ui(!p.fit, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut p.paper_units).range(1e-6..=1e9).speed(0.1));
                        ui.label(tr("paper units ="));
                        ui.add(egui::DragValue::new(&mut p.drawing_units).range(1e-6..=1e9).speed(0.1));
                        ui.label(tr("drawing units"));
                    });
                });
            });
            ui.end_row();
            ui.label(tr("Plot offset"));
            ui.checkbox(&mut p.center, tr("Center the plot"));
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut p.lineweights, tr("Plot object lineweights"));
            ui.end_row();
        });
        if app.ui.plot.area == "window" && app.session.plot_window.is_none() {
            ui.label(tr("Pick a window first."));
        }
        ui.separator();
        ui.horizontal(|ui| {
            let ready = app.ui.plot.area != "window" || app.session.plot_window.is_some();
            if ui.add_enabled(ready, egui::Button::new(tr("OK"))).clicked() {
                go = true;
            }
        });
    });
    if pick {
        app.ui.plot.picking = true;
        *open = false;
        app.start("plot.window");
        return;
    }
    if go {
        *open = false;
        let base = app.session.state().map(|s| s.title.clone()).unwrap_or_else(|_| "Drawing".into());
        let stem = base.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(base);
        let Some(path) = app.services.pick_save.as_ref().and_then(|f| f(&format!("{stem}.pdf"))) else { return };
        let mut q = plot_params(app);
        if let Some(o) = q.as_object_mut() {
            o.insert("path".into(), json!(path));
        }
        match app.session.execute("plot", &q) {
            Ok(_) => app.session.echo(format!("Plotted to {path}")),
            Err(e) => app.session.echo(e.to_string()),
        }
    }
}

// ---------- Text Style ----------

pub fn text_style(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let Ok(d) = app.session.doc() else {
        *open = false;
        return;
    };
    let current = d.header.str("TEXTSTYLE", "Standard");
    let styles = d.text_styles.clone();
    let sel_id = egui::Id::new("textstyle_sel");
    let new_id = egui::Id::new("textstyle_new");
    let mut action: Option<(&'static str, Value)> = None;
    egui::Window::new(tr("Text Style")).open(open).resizable(false).collapsible(false).show(ctx, |ui| {
        let mut sel: String = ui.data_mut(|m| m.get_temp(sel_id)).unwrap_or_else(|| current.clone());
        if !styles.iter().any(|s| s.name.eq_ignore_ascii_case(&sel)) {
            sel = current.clone();
        }
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(tr("Styles"));
                egui::ScrollArea::vertical().max_height(180.0).id_salt("ts_list").show(ui, |ui| {
                    for s in &styles {
                        let label = if s.name.eq_ignore_ascii_case(&current) { format!("{} ✓", s.name) } else { s.name.clone() };
                        if ui.selectable_label(s.name.eq_ignore_ascii_case(&sel), label).clicked() {
                            sel = s.name.clone();
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let Some(st) = styles.iter().find(|s| s.name.eq_ignore_ascii_case(&sel)) else { return };
                let mut font = st.font.clone();
                let mut height = st.height;
                let mut width = st.width_factor;
                let mut oblique = st.oblique.to_degrees();
                egui::Grid::new("ts_grid").num_columns(2).show(ui, |ui| {
                    ui.label(tr("Font name"));
                    if ui.text_edit_singleline(&mut font).lost_focus() && font.trim() != st.font {
                        action = Some(("style", json!({ "name": st.name, "font": font.trim(), "current": false })));
                    }
                    ui.end_row();
                    ui.label(tr("Height"));
                    if ui.add(egui::DragValue::new(&mut height).range(0.0..=1e6).speed(0.1)).changed() {
                        action = Some(("style", json!({ "name": st.name, "height": height, "current": false })));
                    }
                    ui.end_row();
                    ui.label(tr("Width factor"));
                    if ui.add(egui::DragValue::new(&mut width).range(0.01..=100.0).speed(0.01)).changed() {
                        action = Some(("style", json!({ "name": st.name, "widthFactor": width, "current": false })));
                    }
                    ui.end_row();
                    ui.label(tr("Oblique angle"));
                    if ui.add(egui::DragValue::new(&mut oblique).range(-85.0..=85.0).speed(0.5)).changed() {
                        action = Some(("style", json!({ "name": st.name, "oblique": oblique, "current": false })));
                    }
                    ui.end_row();
                });
                ui.label(tr("Height 0 asks for the height each time text is placed."));
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tr("Set Current")).clicked() {
                action = Some(("style.current", json!({ "name": sel })));
            }
            let mut name: String = ui.data_mut(|m| m.get_temp(new_id)).unwrap_or_default();
            ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("New style name")).desired_width(140.0));
            if ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tr("New..."))).clicked() {
                action = Some(("style", json!({ "name": name.trim(), "current": false })));
                sel = name.trim().to_string();
                name.clear();
            }
            ui.data_mut(|m| m.insert_temp(new_id, name));
            if ui.add_enabled(!sel.eq_ignore_ascii_case(&current) && !sel.eq_ignore_ascii_case("Standard"), egui::Button::new(tr("Delete"))).clicked()
            {
                action = Some(("style.delete", json!({ "name": sel })));
            }
        });
        ui.data_mut(|m| m.insert_temp(sel_id, sel));
    });
    if let Some((c, p)) = action {
        let _ = app.run(c, p);
    }
}

// ---------- Dimension Style ----------

pub fn dim_style(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let Ok(d) = app.session.doc() else {
        *open = false;
        return;
    };
    let current = d.header.str("DIMSTYLE", "Standard");
    let styles = d.dim_styles.clone();
    let dimscale = d.header.f64("DIMSCALE", 1.0);
    let sel_id = egui::Id::new("dimstyle_sel");
    let new_id = egui::Id::new("dimstyle_new");
    let mut action: Option<(&'static str, Value)> = None;
    egui::Window::new(tr("Dimension Style Manager")).open(open).resizable(false).collapsible(false).show(ctx, |ui| {
        let mut sel: String = ui.data_mut(|m| m.get_temp(sel_id)).unwrap_or_else(|| current.clone());
        if !styles.iter().any(|s| s.name.eq_ignore_ascii_case(&sel)) {
            sel = current.clone();
        }
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(tr("Styles"));
                egui::ScrollArea::vertical().max_height(220.0).id_salt("ds_list").show(ui, |ui| {
                    for s in &styles {
                        let label = if s.name.eq_ignore_ascii_case(&current) { format!("{} ✓", s.name) } else { s.name.clone() };
                        if ui.selectable_label(s.name.eq_ignore_ascii_case(&sel), label).clicked() {
                            sel = s.name.clone();
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let Some(st) = styles.iter().find(|s| s.name.eq_ignore_ascii_case(&sel)) else { return };
                let name = st.name.clone();
                egui::Grid::new("ds_grid").num_columns(2).show(ui, |ui| {
                    let mut field = |ui: &mut egui::Ui, label: &str, key: &'static str, v: f64, lo: f64, hi: f64, speed: f64| {
                        let mut x = v;
                        ui.label(tr(label));
                        if ui.add(egui::DragValue::new(&mut x).range(lo..=hi).speed(speed)).changed() {
                            // Precision is a whole number of decimals.
                            let v = if key == "decimals" { json!(x.round() as i64) } else { json!(x) };
                            action = Some(("dimstyle", json!({ "name": name, key: v, "current": false })));
                        }
                        ui.end_row();
                    };
                    field(ui, "Text height", "textHeight", st.text_height, 0.0, 1e6, 0.05);
                    field(ui, "Arrow size", "arrowSize", st.arrow_size, 0.0, 1e6, 0.05);
                    field(ui, "Offset from origin", "extOffset", st.ext_offset, 0.0, 1e6, 0.05);
                    field(ui, "Extend beyond dim lines", "extExtend", st.ext_extend, 0.0, 1e6, 0.05);
                    field(ui, "Offset from dim line", "textGap", st.text_gap, 0.0, 1e6, 0.05);
                    field(ui, "Precision (decimals)", "decimals", f64::from(st.decimals), 0.0, 8.0, 0.1);
                    field(ui, "Linear scale factor", "linearFactor", st.linear_factor, 1e-6, 1e6, 0.01);
                    field(ui, "Style scale", "scale", st.scale, 1e-6, 1e6, 0.1);
                });
                ui.separator();
                let mut k = dimscale;
                ui.horizontal(|ui| {
                    ui.label(tr("Overall scale (DIMSCALE)"));
                    if ui.add(egui::DragValue::new(&mut k).range(1e-6..=1e6).speed(0.5)).changed() {
                        action = Some(("setvar", json!({ "name": "DIMSCALE", "value": k })));
                    }
                });
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tr("Set Current")).clicked() {
                action = Some(("dimstyle.current", json!({ "name": sel })));
            }
            let mut name: String = ui.data_mut(|m| m.get_temp(new_id)).unwrap_or_default();
            ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("New style name")).desired_width(140.0));
            if ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tr("New..."))).clicked() {
                action = Some(("dimstyle", json!({ "name": name.trim(), "current": false })));
                sel = name.trim().to_string();
                name.clear();
            }
            ui.data_mut(|m| m.insert_temp(new_id, name));
            if ui.add_enabled(!sel.eq_ignore_ascii_case(&current) && !sel.eq_ignore_ascii_case("Standard"), egui::Button::new(tr("Delete"))).clicked()
            {
                action = Some(("dimstyle.delete", json!({ "name": sel })));
            }
        });
        ui.data_mut(|m| m.insert_temp(sel_id, sel));
    });
    if let Some((c, p)) = action {
        let _ = app.run(c, p);
    }
}
