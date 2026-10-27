//! Keeps the window inside its monitor's work area (the screen minus the taskbar or Dock).
//!
//! Redrob CAD opens at 1440 × 900 pt, which runs past the bottom of a 1366 × 768 screen and under
//! the Windows taskbar, with the title bar (the strip that moves the window) out of reach. egui
//! doesn't expose the work area, only the monitor's size: [`fit_window`] maximizes a window that
//! doesn't fit its monitor less a taskbar-sized margin, which puts it in the work area. Taken from
//! Redrob Canvas (`work_area::fit_window`).

use egui::Vec2;

/// Room kept free at the monitor's bottom edge for a taskbar or Dock egui can't see (the Windows
/// taskbar is 40–48 pt tall; a bottom Dock about as much).
const TASKBAR: f32 = 48.0;

/// Does a window of `outer` size fit on a `monitor` (less the taskbar)?
fn fits(outer: Vec2, monitor: Vec2) -> bool {
    outer.x <= monitor.x + 0.5 && outer.y <= monitor.y - TASKBAR + 0.5
}

fn fitted_id() -> egui::Id {
    egui::Id::new("cc-window-fitted")
}

/// Once, as soon as the monitor is known: maximize a window that is bigger than its monitor
/// (it ran under the taskbar and off-screen). Returns true when it asked to.
pub fn fit_window(ctx: &egui::Context) -> bool {
    if ctx.data(|d| d.get_temp::<bool>(fitted_id())).is_some() {
        return false;
    }
    let content = ctx.content_rect();
    let (monitor, outer, maximized, fullscreen) = ctx.input(|i| {
        let v = i.viewport();
        (v.monitor_size, v.outer_rect, v.maximized, v.fullscreen)
    });
    // On Wayland the window's position, and so its outer rect, is never known: its content size
    // is the least the window takes up.
    let outer = outer.unwrap_or(content);
    let Some(m) = monitor else { return false };
    // macOS keeps new windows inside the screen's visible frame itself.
    if cfg!(target_os = "macos") {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(fitted_id(), true));
    if maximized == Some(true) || fullscreen == Some(true) || !m.is_finite() || !outer.is_finite() || fits(outer.size(), m) {
        return false;
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
    true
}
