//! CADCraft desktop app.
//!
//! Usage: `cadcraft [--control <port>] [--sample] [files…]`
//!
//! `--control <port>` (or `CADCRAFT_CONTROL_PORT`) starts a localhost JSON-lines control server:
//! `{"id":1,"method":"cmdline.input","params":{"text":"circle 0,0 5"}}` → `{"id":1,"ok":true,…}`.
//! See `cadcraft_ui_egui::control` for the methods.
//!
//! Without `--control`, the server still runs on a free port with a per-launch token, for the
//! built-in agent only (see `acp`). `redrob-cad mcp --connect <addr>` is the MCP server that agent
//! is given: it serves MCP on stdio and drives the window at `<addr>`.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod acp;
mod control_server;
mod device;
#[cfg(target_os = "macos")]
mod native_menu;
mod work_area;

use cadcraft_engine::Session;
use cadcraft_ui_egui::{CadApp, Services};

struct App(CadApp, #[cfg(target_os = "macos")] Option<(native_menu::NativeMenu, cadcraft_ui_egui::i18n::Lang)>);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        {
            // (Re)built at launch and whenever the interface language changes, so its titles follow.
            let lang = cadcraft_ui_egui::i18n::lang();
            if self.1.as_ref().is_none_or(|(_, l)| *l != lang) && std::env::var_os("CADCRAFT_NO_NATIVE_MENU").is_none() {
                self.1 = Some((native_menu::NativeMenu::install(&mut self.0), lang));
            }
            if let Some((m, _)) = &mut self.1 {
                m.poll(&mut self.0, ctx);
            }
        }
        // Once: a window bigger than its monitor is maximized into the work area, so its title
        // bar can be reached.
        work_area::fit_window(ctx);
        self.0.logic(ctx);
        if self.0.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(raw);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

/// The interface language: `REDROB_CAD_LANG`, else the one saved from the Language menu, else
/// the platform's (POSIX locale variables, then the OS setting).
fn startup_language() -> cadcraft_ui_egui::i18n::Lang {
    use cadcraft_ui_egui::i18n::{self, Lang};
    if let Some(l) = std::env::var("REDROB_CAD_LANG").ok().and_then(|v| Lang::from_code(&v)) {
        return l;
    }
    let saved = settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("language").and_then(serde_json::Value::as_str).and_then(Lang::from_code));
    saved.unwrap_or_else(|| i18n::detect(sys_locale::get_locale().as_deref()))
}

fn settings_path() -> Option<std::path::PathBuf> {
    device::config_dir().map(|d| d.join("settings.json"))
}

/// Writes the language into `settings.json`, keeping any other keys there.
fn save_language(l: cadcraft_ui_egui::i18n::Lang) {
    let Some(path) = settings_path() else { return };
    let mut v =
        std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    if let Some(o) = v.as_object_mut() {
        o.insert("language".into(), serde_json::json!(l.code()));
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, format!("{v:#}\n")) {
        log::warn!("saving the language: {e}");
    }
}

/// File format hooks for the engine (it stays I/O-agnostic).
pub fn install_io() {
    cadcraft_engine::cmd::file::set_io(cadcraft_engine::cmd::file::IoHooks {
        read: |b, name| cadcraft_io::read(b, name).map_err(|e| e.to_string()),
        write: |d, name| cadcraft_io::write(d, name).map_err(|e| e.to_string()),
        plot: Some(|d, space, opts| cadcraft_io::plot(d, space, opts).map_err(|e| e.to_string())),
    });
}

fn services(agent: Option<cadcraft_ui_egui::agent::AgentFactory>, account: Option<Box<dyn cadcraft_ui_egui::account::AccountService>>) -> Services {
    Services {
        agent,
        account,
        save_language: Some(Box::new(save_language)),
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Drawing (DXF)", &["dxf"])
                .add_filter("All files", &["*"])
                .pick_file()
                .map(|p| p.to_string_lossy().to_string())
        })),
        pick_save: Some(Box::new(|name: &str| {
            rfd::FileDialog::new()
                .set_file_name(name)
                .add_filter("Drawing (DXF)", &["dxf"])
                .add_filter("SVG", &["svg"])
                .add_filter("PNG", &["png"])
                .save_file()
                .map(|p| p.to_string_lossy().to_string())
        })),
    }
}

const APP_ID: &str = "ai.redrob.cad";

fn app_icon() -> Option<egui::IconData> {
    let png: &[u8] = include_bytes!("../../../assets/app-icon/redrob-cad-256.png");
    eframe::icon_data::from_png_bytes(png).map_err(|e| log::warn!("app icon: {e}")).ok()
}

fn version_string() -> String {
    let sha = option_env!("CADCRAFT_BUILD_SHA").unwrap_or("dev");
    format!("redrob-cad {} ({sha})", env!("CARGO_PKG_VERSION"))
}

/// A per-launch secret for the control server: 128 bits from the standard library's randomly
/// seeded hasher (no extra dependency for one value).
fn control_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2u8 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u8(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Where the agent works: the launch directory, or the home directory when launched from a
/// desktop shell (whose working directory is `/`) or from the install folder (a Windows shortcut
/// starts the app in the folder that holds `redrob-cad.exe`): the agent never works there.
fn agent_cwd() -> String {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from);
    let exe_dir =
        std::env::current_exe().ok().and_then(|e| e.canonicalize().ok().or(Some(e))).and_then(|e| e.parent().map(std::path::Path::to_path_buf));
    let cwd = std::env::current_dir().ok().filter(|d| d.parent().is_some()).filter(|d| !is_install_dir(d, exe_dir.as_deref(), home.as_deref()));
    cwd.or(home).map(|d| d.to_string_lossy().to_string()).unwrap_or_else(|| ".".into())
}

/// Whether `dir` is the folder holding the executable, or one of its parents below the home
/// directory or the drive root (`…\Redrob CAD\bin` → `…\Redrob CAD`, `…\Programs`).
fn is_install_dir(dir: &std::path::Path, exe_dir: Option<&std::path::Path>, home: Option<&std::path::Path>) -> bool {
    let Some(exe_dir) = exe_dir else { return false };
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let home = home.map(|h| h.canonicalize().unwrap_or_else(|_| h.to_path_buf()));
    exe_dir.ancestors().take_while(|a| a.parent().is_some() && Some(*a) != home.as_deref()).any(|a| a == dir)
}

/// Whether eframe's saved state (`ui.ron`) holds a window position from an earlier session. The
/// window entry is a RON string inside the file, so a text match is enough; eframe restores the
/// inner position on macOS and the outer one elsewhere.
fn has_saved_position(path: Option<&std::path::Path>) -> bool {
    let Some(text) = path.and_then(|p| std::fs::read_to_string(p).ok()) else {
        return false;
    };
    let key = if cfg!(target_os = "macos") { "inner_position_pixels:Some(" } else { "outer_position_pixels:Some(" };
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains(key)
}

/// `redrob-cad mcp --connect <addr>`: the MCP server the built-in agent runs.
fn serve_mcp(args: &[String]) -> std::process::ExitCode {
    let Some(addr) = args.iter().position(|a| a == "--connect").and_then(|i| args.get(i + 1)) else {
        eprintln!("usage: redrob-cad mcp --connect HOST:PORT");
        return std::process::ExitCode::FAILURE;
    };
    let backend = match cadcraft_mcp::Remote::connect(addr) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("redrob-cad: cannot connect to {addr}: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut server = cadcraft_mcp::Server::new(Box::new(backend));
    match server.serve(std::io::stdin().lock(), std::io::stdout().lock()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("redrob-cad: mcp: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn main() -> eframe::Result {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("mcp") {
        let code = serve_mcp(&argv[1..]);
        std::process::exit(if code == std::process::ExitCode::SUCCESS { 0 } else { 1 });
    }
    let mut control_port: Option<u16> = std::env::var("CADCRAFT_CONTROL_PORT").ok().and_then(|p| p.parse().ok());
    let mut files = Vec::new();
    let mut sample = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--control" => control_port = args.next().and_then(|p| p.parse().ok()),
            "--sample" => sample = true,
            "--version" | "-V" => {
                println!("{}", version_string());
                return Ok(());
            }
            _ => files.push(a),
        }
    }
    install_io();
    cadcraft_ui_egui::i18n::set_lang(startup_language());
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Redrob CAD")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_app_id(APP_ID),
        ..Default::default()
    };
    if let Some(icon) = app_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }
    // Centre only a window with no saved position; the first frame then maximizes one that
    // doesn't fit its monitor (`work_area::fit_window`).
    options.centered = !has_saved_position(options.persistence_path.as_deref());
    eframe::run_native(
        "Redrob CAD",
        options,
        Box::new(move |cc| {
            // `--control PORT` keeps the tokenless protocol; otherwise a free port and a token.
            let token = if control_port.is_some() { None } else { Some(control_token()) };
            let (rx, bound) = control_server::start(control_port.unwrap_or(0), cc.egui_ctx.clone(), token.clone());
            let base = std::env::var(device::CONSOLE_API_ENV).ok().filter(|b| !b.is_empty()).unwrap_or_else(|| device::CONSOLE_API.to_string());
            let account = device::DeviceAccount::new(
                device::config_dir().map(|d| d.join("account.json")),
                device::shared_account_path(),
                std::sync::Arc::new(device::Https),
                base,
                Some(cc.egui_ctx.clone()),
            );
            let keys = account.key_source();
            let agent: Option<cadcraft_ui_egui::agent::AgentFactory> = bound.map(|port| {
                let link = acp::McpLink { addr: format!("127.0.0.1:{port}"), token };
                let ctx = cc.egui_ctx.clone();
                Box::new(move || {
                    let bin = acp::find_binary().ok_or_else(|| acp::INSTALL_HINT.to_string())?;
                    // The connected workspace's key, as the variable Redrob Code reads for Redrob.
                    let env: Vec<(&str, String)> = keys.current().map(|(k, _)| vec![("REDROB_API_KEY", k.api_key)]).unwrap_or_default();
                    let host = acp::AcpHost::start(&bin, &agent_cwd(), link.clone(), &env, ctx.clone())?;
                    Ok(Box::new(host) as Box<dyn cadcraft_ui_egui::agent::AgentHost>)
                }) as cadcraft_ui_egui::agent::AgentFactory
            });
            let mut app = CadApp::new(Session::empty(), services(agent, Some(Box::new(account))));
            app.integrated_titlebar = cfg!(target_os = "macos");
            if let Some(rs) = &cc.wgpu_render_state {
                app.set_wgpu(rs);
            }
            app = app.with_control(rx);
            if sample {
                let _ = app.run("ui.sample", serde_json::json!({}));
            }
            for f in &files {
                app.open_path(f);
            }
            if app.session.docs.is_empty() {
                app.session.new_drawing(false);
            }
            Ok(Box::new(App(
                app,
                #[cfg(target_os = "macos")]
                None,
            )))
        }),
    )
}
