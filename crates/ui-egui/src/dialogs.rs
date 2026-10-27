//! Dialogs: Drafting Settings, About, command reference, blocks; dispatches the Layer Properties
//! Manager ([`crate::layers`]), Quick Select ([`crate::quick`]) and Parameters Manager
//! ([`crate::parametric`]).

use crate::i18n::{tr, trf};
use egui::{RichText, vec2};

use crate::CadApp;
use crate::theme::Tokens;

pub fn show(app: &mut CadApp, ctx: &egui::Context) {
    mtext_editor(app, ctx);
    crate::quick::quick_properties(app, ctx);
    crate::styles::tick(app);
    let Some(d) = app.ui.dialog.clone() else { return };
    let mut open = true;
    match d.as_str() {
        "layers" => crate::layers::dialog(app, ctx, &mut open),
        "qselect" => crate::quick::qselect_dialog(app, ctx, &mut open),
        "parameters" => crate::parametric::parameters_dialog(app, ctx, &mut open),
        "dsettings" => dsettings(app, ctx, &mut open),
        "about" => about(ctx, &mut open),
        "commands" => commands(app, ctx, &mut open),
        "blocks" => blocks(app, ctx, &mut open),
        "plot" => crate::styles::plot(app, ctx, &mut open),
        "textstyle" => crate::styles::text_style(app, ctx, &mut open),
        "dimstyle" => crate::styles::dim_style(app, ctx, &mut open),
        _ => open = false,
    }
    if !open {
        app.ui.dialog = None;
    }
}

/// Drafting Settings (DSETTINGS / DS / SE; OSNAP opens the Object Snap tab). Edits a copy;
/// OK applies it, Cancel or closing the window drops it, as in AutoCAD.
fn dsettings(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    use redrob_cad_engine::snap::mode;
    let draft_id = egui::Id::new("dsettings_draft");
    let mut draft: redrob_cad_engine::Settings = ctx.data_mut(|d| d.get_temp(draft_id)).unwrap_or_else(|| app.session.settings.clone());
    let mut tab = app.ui.dsettings_tab.min(4);
    let mut done: Option<bool> = None;
    egui::Window::new(tr("Drafting Settings")).open(open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (i, l) in ["Snap and Grid", "Object Snap", "Polar Tracking", "Dynamic Input", "Quick Properties"].iter().enumerate() {
                if ui.selectable_label(tab == i as u8, tr(l)).clicked() {
                    tab = i as u8;
                }
            }
        });
        ui.separator();
        let s = &mut draft;
        match tab {
            0 => {
                ui.checkbox(&mut s.snapmode, tr("Snap On (F9)"));
                egui::Grid::new("ds_snap").num_columns(2).show(ui, |ui| {
                    ui.label(tr("Snap X spacing"));
                    ui.add(egui::DragValue::new(&mut s.snapunit.x).speed(0.05).range(0.0001..=1e6));
                    ui.end_row();
                    ui.label(tr("Snap Y spacing"));
                    ui.add(egui::DragValue::new(&mut s.snapunit.y).speed(0.05).range(0.0001..=1e6));
                    ui.end_row();
                });
                ui.separator();
                ui.checkbox(&mut s.gridmode, tr("Grid On (F7)"));
                egui::Grid::new("ds_grid").num_columns(2).show(ui, |ui| {
                    ui.label(tr("Grid X spacing"));
                    ui.add(egui::DragValue::new(&mut s.gridunit.x).speed(0.05).range(0.0001..=1e6));
                    ui.end_row();
                    ui.label(tr("Grid Y spacing"));
                    ui.add(egui::DragValue::new(&mut s.gridunit.y).speed(0.05).range(0.0001..=1e6));
                    ui.end_row();
                    ui.label(tr("Major line every"));
                    ui.add(egui::DragValue::new(&mut s.gridmajor).range(1..=100));
                    ui.end_row();
                });
            }
            1 => {
                ui.horizontal(|ui| {
                    let mut on = s.osmode & mode::OFF == 0;
                    if ui.checkbox(&mut on, tr("Object Snap On (F3)")).changed() {
                        if on {
                            s.osmode &= !mode::OFF;
                        } else {
                            s.osmode |= mode::OFF;
                        }
                    }
                    ui.add_space(40.0);
                    ui.checkbox(&mut s.otrack, tr("Object Snap Tracking On (F11)"));
                });
                ui.group(|ui| {
                    ui.label(tr("Object Snap modes"));
                    // AutoCAD's two columns, in its order.
                    let left = [mode::END, mode::MID, mode::CEN, mode::GCEN, mode::NOD, mode::QUA, mode::INT, mode::EXT];
                    let right = [mode::INS, mode::PER, mode::TAN, mode::NEA, mode::APP, mode::PAR];
                    let name = |m: u32| mode::ALL.iter().find(|(b, _)| *b == m).map(|(_, n)| *n).unwrap_or("");
                    let snap_col = Tokens::get().snap;
                    ui.horizontal_top(|ui| {
                        for col in [&left[..], &right[..]] {
                            ui.vertical(|ui| {
                                for &m in col {
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                                        crate::canvas::snap_glyph(ui.painter(), r.center(), m, 5.0, egui::Stroke::new(1.3, snap_col));
                                        let mut v = s.osmode & m != 0;
                                        if ui.checkbox(&mut v, tr(name(m))).changed() {
                                            if v {
                                                s.osmode |= m;
                                            } else {
                                                s.osmode &= !m;
                                            }
                                        }
                                    });
                                }
                            });
                            ui.add_space(24.0);
                        }
                        ui.vertical(|ui| {
                            let all: u32 = mode::ALL.iter().map(|(b, _)| *b).fold(0, |a, b| a | b);
                            if ui.add_sized(vec2(90.0, 22.0), egui::Button::new(tr("Select All"))).clicked() {
                                s.osmode = (s.osmode & mode::OFF) | all;
                            }
                            if ui.add_sized(vec2(90.0, 22.0), egui::Button::new(tr("Clear All"))).clicked() {
                                s.osmode &= mode::OFF;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(tr("To track from an Osnap point, pause over the point while in a command. A tracking vector appears when you move the cursor. To stop tracking, pause over the point again."));
                });
            }
            2 => {
                ui.checkbox(&mut s.polarmode, tr("Polar Tracking On (F10)"));
                let mut deg = s.polarang.to_degrees();
                ui.horizontal(|ui| {
                    ui.label(tr("Increment angle"));
                    egui::ComboBox::from_id_salt("polarang").selected_text(format!("{deg}")).show_ui(ui, |ui| {
                        for a in [90.0, 45.0, 30.0, 22.5, 18.0, 15.0, 10.0, 5.0] {
                            ui.selectable_value(&mut deg, a, format!("{a}"));
                        }
                    });
                });
                s.polarang = deg.to_radians();
                ui.checkbox(&mut s.orthomode, tr("Ortho (F8)"));
            }
            3 => {
                ui.checkbox(&mut s.dynmode, tr("Enable Dynamic Input (F12)"));
            }
            _ => {
                ui.checkbox(&mut s.qpmode, tr("Show the Quick Properties palette on selection"));
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.add_sized(vec2(80.0, 24.0), egui::Button::new(tr("OK"))).clicked() {
                done = Some(true);
            }
            if ui.add_sized(vec2(80.0, 24.0), egui::Button::new(tr("Cancel"))).clicked() {
                done = Some(false);
            }
        });
    });
    app.ui.dsettings_tab = tab;
    match done {
        Some(apply) => {
            if apply {
                app.session.settings = draft;
                app.session.touch();
            }
            ctx.data_mut(|d| d.remove::<redrob_cad_engine::Settings>(draft_id));
            *open = false;
        }
        None if !*open => {
            ctx.data_mut(|d| d.remove::<redrob_cad_engine::Settings>(draft_id));
        }
        None => {
            ctx.data_mut(|d| d.insert_temp(draft_id, draft));
        }
    }
}

fn about(ctx: &egui::Context, open: &mut bool) {
    let t = Tokens::get();
    let tab_id = egui::Id::new("about_tab");
    egui::Window::new(tr("About Redrob CAD")).open(open).default_size(vec2(640.0, 420.0)).collapsible(false).show(ctx, |ui| {
        let mut tab = ui.data_mut(|d| d.get_temp::<u8>(tab_id)).unwrap_or(0);
        ui.horizontal(|ui| {
            for (i, l) in ["About", "Contributors", "Models"].iter().enumerate() {
                if ui.selectable_label(tab == i as u8, tr(l)).clicked() {
                    tab = i as u8;
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(tab_id, tab));
        ui.separator();
        match tab {
            1 => crate::credits::contributors_ui(ui),
            2 => crate::credits::models_ui(ui),
            _ => {
                ui.heading("Redrob CAD");
                ui.label(trf("Version {version}", &[("version", env!("CARGO_PKG_VERSION"))]));
                ui.label(tr("Computer-aided design and drafting: an open-source, clean-room CAD application written in pure Rust."));
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Made by Redrob and contributors. See NOTICE for where the code came from.")).color(t.text_dim));
                ui.hyperlink_to(tr("github.com/redrob-labs/redrob-cad"), "https://github.com/redrob-labs/redrob-cad");
                ui.hyperlink_to(tr("Report an issue"), "https://github.com/redrob-labs/redrob-cad/issues");
                ui.add_space(6.0);
                ui.label(RichText::new(tr("MIT OR Apache-2.0. Not affiliated with Autodesk, Inc.")).small().color(t.text_faint));
            }
        }
    });
}

fn commands(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let mut start = None;
    egui::Window::new(tr("Command Reference")).open(open).default_size(vec2(640.0, 480.0)).show(ctx, |ui| {
        let id = ui.id().with("cmdfilter");
        let mut filter = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(tr("Filter"));
            ui.text_edit_singleline(&mut filter);
        });
        ui.data_mut(|d| d.insert_temp(id, filter.clone()));
        let f = filter.to_ascii_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("cmdref").striped(true).num_columns(3).show(ui, |ui| {
                for c in redrob_cad_engine::command_specs() {
                    if !f.is_empty() && !c.id.contains(&f) && !c.label.to_ascii_lowercase().contains(&f) {
                        continue;
                    }
                    if ui.link(c.id.to_ascii_uppercase()).clicked() {
                        start = Some(c.id);
                    }
                    ui.label(c.label);
                    ui.label(RichText::new(if c.aliases.is_empty() { String::new() } else { c.aliases.join(", ").to_ascii_uppercase() }).small());
                    ui.end_row();
                }
            });
        });
    });
    if let Some(c) = start {
        app.ui.dialog = None;
        app.start(c);
    }
}

fn blocks(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    egui::Window::new(tr("Blocks")).open(open).default_size(vec2(320.0, 360.0)).show(ctx, |ui| {
        let Ok(d) = app.session.doc() else { return };
        let names: Vec<&String> = d.blocks.keys().filter(|k| !k.starts_with('*')).collect();
        if names.is_empty() {
            ui.label(tr("No blocks defined in this drawing."));
        }
        for n in names {
            ui.label(n);
        }
    });
}

/// The multiline text editor shown while MTEXT asks for its contents.
fn mtext_editor(app: &mut CadApp, ctx: &egui::Context) {
    let active = app.session.running.as_ref().is_some_and(|r| r.id == "mtext")
        && app.session.current_prompt().is_some_and(|p| p.accept.text && !p.accept.point);
    let id = egui::Id::new("mtext_editor_buffer");
    if !active {
        ctx.data_mut(|d| d.remove::<String>(id));
        return;
    }
    let mut buf = ctx.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
    let mut submit = None;
    let mut cancel = false;
    egui::Window::new(tr("Text Editor")).collapsible(false).resizable(true).default_size(vec2(460.0, 220.0)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Style: Standard"));
            ui.separator();
            let h = app.session.doc().map(|d| d.header.f64("TEXTSIZE", 0.2)).unwrap_or(0.2);
            ui.label(trf("Height: {height}", &[("height", format!("{h:.4}").as_str())]));
            ui.separator();
            if ui.button(tr("B")).on_hover_text(tr("Bold")).clicked() {
                buf.push_str("{\\fArial|b1;}");
            }
            if ui.button("⅟").on_hover_text(tr("Stack (type 1/2 then select)")).clicked() {
                buf.push_str("\\S1/2;");
            }
            if ui.button("°").on_hover_text(tr("Degree")).clicked() {
                buf.push_str("%%d");
            }
            if ui.button("±").on_hover_text(tr("Plus/minus")).clicked() {
                buf.push_str("%%p");
            }
            if ui.button("⌀").on_hover_text(tr("Diameter")).clicked() {
                buf.push_str("%%c");
            }
        });
        let r = ui.add(
            egui::TextEdit::multiline(&mut buf).desired_rows(6).desired_width(f32::INFINITY).hint_text(tr("Type text; Enter starts a new paragraph")),
        );
        if !r.has_focus() && buf.is_empty() {
            r.request_focus();
        }
        ui.horizontal(|ui| {
            if ui.button(tr("OK")).clicked() || (r.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))) {
                submit = Some(buf.replace('\n', "\\P"));
            }
            if ui.button(tr("Cancel")).clicked() {
                cancel = true;
            }
            ui.label(RichText::new(tr("⌘↩ to finish")).small());
        });
    });
    ctx.data_mut(|d| d.insert_temp(id, buf));
    if let Some(t) = submit {
        ctx.data_mut(|d| d.remove::<String>(id));
        let _ = app.session.input(redrob_cad_engine::Input::Text(t));
    } else if cancel {
        ctx.data_mut(|d| d.remove::<String>(id));
        app.session.cancel();
    }
}
