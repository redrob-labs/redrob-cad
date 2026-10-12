//! The Redrob workspace this app is connected to, as the Agent palette shows it.
//!
//! Drafting never needs an account. The built-in agent does: its model calls are billed to a
//! Redrob workspace. Connecting uses the console's device flow, so nobody copies a key: the app
//! shows a short code, the person approves it on console.redrob.ai, and the console hands this
//! machine a workspace key. The flow and the stored key live behind [`AccountService`], supplied by
//! the desktop app through [`crate::Services::account`]; the UI only ever sees names, never the key.

use crate::i18n::{tr, trf};
use egui::{RichText, vec2};

use crate::theme::Tokens;

/// What may be shown about the connected workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub account_name: String,
    pub key_name: String,
    /// The key is the computer's shared one, from Redrob Launcher: signing out happens there.
    pub via_launcher: bool,
}

/// A connection the person is approving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingConnect {
    pub user_code: String,
    /// The approval page with the code filled in.
    pub url: String,
    /// Seconds left before the code expires.
    pub seconds_left: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectState {
    Idle,
    Starting,
    Waiting(PendingConnect),
    /// The attempt ended without a key; the text says why in plain words.
    Failed(String),
}

/// The desktop app's account store and device-flow client. Every call returns at once.
pub trait AccountService {
    fn account(&self) -> Option<Account>;
    fn state(&mut self) -> ConnectState;
    fn connect(&mut self);
    fn cancel(&mut self);
    fn disconnect(&mut self);
    /// Bumped whenever the stored key changes, so a running agent can be restarted with it.
    fn generation(&self) -> u64;
}

/// The palette's account row. Returns true while a connection is waiting, so the caller repaints.
pub fn account_row(service: &mut Option<Box<dyn AccountService>>, ui: &mut egui::Ui) -> bool {
    let t = Tokens::get();
    let Some(svc) = service.as_mut() else { return false };
    let state = svc.state();
    let account = svc.account();
    let mut action: Option<fn(&mut dyn AccountService)> = None;
    // The caller's frame supplies the side margins.
    ui.horizontal_wrapped(|ui| {
        match (&account, &state) {
            (_, ConnectState::Starting) => {
                ui.spinner();
                ui.label(RichText::new(tr("Asking console.redrob.ai for a code…")).small().color(t.text_dim));
            }
            (_, ConnectState::Waiting(p)) => {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("Approve this code on console.redrob.ai:")).small().color(t.text_dim));
                    ui.label(RichText::new(&p.user_code).size(20.0).monospace().color(t.text));
                    ui.horizontal(|ui| {
                        ui.hyperlink_to(tr("Open console.redrob.ai"), &p.url);
                        ui.label(
                            RichText::new(trf("{time} left", &[("time", format!("{}:{:02}", p.seconds_left / 60, p.seconds_left % 60).as_str())]))
                                .small()
                                .color(t.text_faint),
                        );
                        if ui.small_button(tr("Cancel")).clicked() {
                            action = Some(|s| s.cancel());
                        }
                    });
                });
            }
            (Some(a), _) => {
                // The key name sits on its own line: a hover tooltip covered the panel text below it.
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(trf("Workspace: {name}", &[("name", a.account_name.as_str())])).small().color(t.text_dim));
                        if a.via_launcher {
                            ui.label(RichText::new(tr("Signed in with Redrob Launcher")).small().color(t.text_faint));
                        } else if ui.small_button(tr("Disconnect")).clicked() {
                            action = Some(|s| s.disconnect());
                        }
                    });
                    ui.label(RichText::new(trf("Key: {name}", &[("name", a.key_name.as_str())])).small().color(t.text_faint));
                });
            }
            (None, s) => {
                if let ConnectState::Failed(why) = s {
                    ui.label(RichText::new(crate::i18n::tr_runtime(why)).small().color(t.text_dim));
                }
                if ui.add_sized(vec2(170.0, 22.0), egui::Button::new(tr("Connect a Redrob workspace"))).clicked() {
                    action = Some(|s| s.connect());
                }
            }
        }
    });
    if let Some(f) = action {
        f(svc.as_mut());
    }
    matches!(state, ConnectState::Starting | ConnectState::Waiting(_))
}
