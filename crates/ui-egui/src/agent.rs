//! The Agent palette: a chat with Redrob Code, which drives this window through its own MCP tools.
//!
//! The UI crate stays platform-neutral (it also runs in the browser): the agent process and its
//! protocol live behind [`AgentHost`], supplied by the desktop app through
//! [`crate::Services::agent`]. Without one (the web build) the palette says so.

use crate::i18n::{tr, tr_runtime, trf};
use egui::{Color32, RichText, vec2};

use crate::CadApp;
use crate::theme::Tokens;

/// One choice in a permission request, as the agent offered it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionOption {
    pub id: String,
    pub name: String,
    /// `allow_once`, `allow_always`, `reject_once` or `reject_always`.
    pub kind: String,
}

/// What the agent process reports, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// The session is open; `name` is the agent's own name and version.
    Ready { name: String },
    /// A piece of the agent's reply.
    Text(String),
    /// A piece of the agent's visible reasoning.
    Thought(String),
    /// A tool call started or changed state.
    Tool { id: String, title: String, status: String },
    /// The agent asks before running a tool; answer with [`AgentHost::answer`].
    Permission { request: u64, title: String, options: Vec<PermissionOption> },
    /// The turn ended; `stop` is the protocol's stop reason (`end_turn`, `cancelled`, ...).
    TurnEnd { stop: String },
    /// Something the user should read that is not part of the reply.
    Notice(String),
    /// The turn or the session failed.
    Error(String),
}

/// A running agent session. Every call returns at once; results arrive through [`Self::poll`].
pub trait AgentHost {
    fn prompt(&mut self, text: &str);
    fn cancel(&mut self);
    /// Answer a [`AgentEvent::Permission`]; `None` declines.
    fn answer(&mut self, request: u64, option: Option<&str>);
    fn poll(&mut self) -> Vec<AgentEvent>;
}

/// Starts an agent session on first use.
pub type AgentFactory = Box<dyn FnMut() -> Result<Box<dyn AgentHost>, String>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    User(String),
    Agent(String),
    Thought(String),
    Tool { id: String, title: String, status: String },
    Notice(String),
    Error(String),
}

#[derive(Default)]
pub struct AgentPanel {
    host: Option<Box<dyn AgentHost>>,
    pub entries: Vec<Entry>,
    pub input: String,
    pub busy: bool,
    pub agent_name: Option<String>,
    pub pending: Option<(u64, String, Vec<PermissionOption>)>,
    /// The account generation the running host was started with.
    generation: u64,
}

impl AgentPanel {
    /// Fold one event into the transcript. Text and thought chunks extend the open entry.
    pub fn apply(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::Ready { name } => self.agent_name = Some(name),
            AgentEvent::Text(s) => match self.entries.last_mut() {
                Some(Entry::Agent(t)) => t.push_str(&s),
                _ => self.entries.push(Entry::Agent(s)),
            },
            AgentEvent::Thought(s) => match self.entries.last_mut() {
                Some(Entry::Thought(t)) => t.push_str(&s),
                _ => self.entries.push(Entry::Thought(s)),
            },
            AgentEvent::Tool { id, title, status } => {
                let found = self.entries.iter_mut().rev().find_map(|e| match e {
                    Entry::Tool { id: i, title: t, status: s } if *i == id => Some((t, s)),
                    _ => None,
                });
                match found {
                    Some((t, s)) => {
                        if !title.is_empty() {
                            *t = title;
                        }
                        if !status.is_empty() {
                            *s = status;
                        }
                    }
                    None => self.entries.push(Entry::Tool { id, title, status }),
                }
            }
            AgentEvent::Permission { request, title, options } => self.pending = Some((request, title, options)),
            AgentEvent::TurnEnd { stop } => {
                self.busy = false;
                self.pending = None;
                if stop != "end_turn" {
                    self.entries.push(Entry::Notice(match stop.as_str() {
                        "cancelled" => "Stopped.".to_string(),
                        "max_tokens" => "The reply hit its length limit.".to_string(),
                        "refusal" => "Redrob Code declined this request.".to_string(),
                        other => format!("The turn ended: {other}."),
                    }));
                }
            }
            AgentEvent::Notice(s) => self.entries.push(Entry::Notice(s)),
            AgentEvent::Error(s) => {
                self.busy = false;
                self.pending = None;
                self.entries.push(Entry::Error(s));
            }
        }
    }

    /// A different workspace key: the next message starts a fresh agent that uses it.
    pub fn account_changed(&mut self, generation: u64) {
        if generation == self.generation {
            return;
        }
        self.generation = generation;
        if self.host.take().is_some() {
            self.busy = false;
            self.pending = None;
            self.agent_name = None;
            self.entries.push(Entry::Notice("Workspace changed. The next message starts the agent with it.".into()));
        }
    }

    fn send(&mut self, factory: &mut Option<AgentFactory>) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.busy {
            return;
        }
        if self.host.is_none() {
            let Some(f) = factory.as_mut() else {
                self.entries.push(Entry::Error("The agent runs in the desktop app only.".into()));
                return;
            };
            match f() {
                Ok(h) => self.host = Some(h),
                Err(e) => {
                    self.entries.push(Entry::Error(e));
                    return;
                }
            }
        }
        self.input.clear();
        self.entries.push(Entry::User(text.clone()));
        self.busy = true;
        if let Some(h) = self.host.as_mut() {
            h.prompt(&text);
        }
    }

    /// Drain the host. Returns true while a turn is running, so the caller keeps repainting.
    pub fn poll(&mut self) -> bool {
        if let Some(h) = self.host.as_mut() {
            for ev in h.poll() {
                self.apply(ev);
            }
        }
        self.busy || self.pending.is_some()
    }

    fn answer(&mut self, option: Option<String>) {
        if let (Some((req, _, _)), Some(h)) = (self.pending.take(), self.host.as_mut()) {
            h.answer(req, option.as_deref());
        }
    }

    fn cancel(&mut self) {
        if let Some(h) = self.host.as_mut() {
            h.cancel();
        }
    }
}

/// Side margin of the chat, in points. One value so the header, log and composer line up.
const MARGIN: i8 = 12;
/// Gap between two messages.
const GAP: f32 = 8.0;
/// A user bubble takes at most this share of the log's width.
const BUBBLE_SHARE: f32 = 0.85;

/// The palette body.
pub fn agent_section(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(MARGIN, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Agent")).size(14.0).color(t.text));
            if let Some(n) = &app.agent.agent_name {
                ui.label(RichText::new(n).small().color(t.text_faint));
            }
        });
        if let Some(g) = app.services.account.as_ref().map(|a| a.generation()) {
            app.agent.account_changed(g);
        }
        if crate::account::account_row(&mut app.services.account, ui) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        ui.add_space(6.0);
        ui.separator();
        chat(app, ui);
    });
}

fn chat(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let panel = &mut app.agent;
    let composer_h = 92.0;
    let list_h = (ui.available_height() - composer_h).max(80.0);
    egui::ScrollArea::vertical().id_salt("agent_log").max_height(list_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        ui.add_space(GAP);
        if panel.entries.is_empty() {
            ui.add(egui::Label::new(RichText::new(tr("Ask Redrob Code to draw, change or check this drawing.")).color(t.text_dim)).wrap());
        }
        let full = ui.available_width();
        for e in &panel.entries {
            match e {
                Entry::User(s) => {
                    // The user's own words sit on the right, in a bubble.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        egui::Frame::NONE.fill(t.control).corner_radius(10).inner_margin(egui::Margin::symmetric(10, 7)).show(ui, |ui| {
                            ui.set_max_width(full * BUBBLE_SHARE);
                            // The bubble hugs the right edge, but its text still reads left to right.
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.add(egui::Label::new(RichText::new(s).color(t.text)).wrap());
                            });
                        });
                    });
                }
                Entry::Agent(s) => {
                    ui.add(egui::Label::new(RichText::new(s).color(t.text)).wrap().selectable(true));
                }
                Entry::Thought(s) => {
                    ui.add(egui::Label::new(RichText::new(s).italics().small().color(t.text_faint)).wrap());
                }
                Entry::Tool { title, status, .. } => {
                    let mark = match status.as_str() {
                        "completed" => "✓",
                        "failed" => "✗",
                        _ => "…",
                    };
                    egui::Frame::NONE
                        .fill(t.chrome_dark)
                        .stroke(egui::Stroke::new(1.0, t.border))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            // Proportional font: the monospace one has no ✓ / ✗ glyphs.
                            ui.add(egui::Label::new(RichText::new(format!("{mark} {title}")).small().color(t.text_dim)).wrap());
                        });
                }
                Entry::Notice(s) => {
                    ui.add(egui::Label::new(RichText::new(tr_runtime(s)).small().color(t.text_dim)).wrap());
                }
                Entry::Error(s) => {
                    ui.add(egui::Label::new(RichText::new(tr_runtime(s)).small().color(Color32::from_rgb(0xff, 0x8a, 0x80))).wrap());
                }
            }
            ui.add_space(GAP);
        }
    });
    let mut answer: Option<Option<String>> = None;
    if let Some((_, title, options)) = &panel.pending {
        ui.add(egui::Label::new(RichText::new(trf("Allow {tool}?", &[("tool", title)])).color(t.text)).wrap());
        ui.horizontal_wrapped(|ui| {
            for o in options {
                if ui.button(&o.name).clicked() {
                    answer = Some(if o.kind.starts_with("reject") { None } else { Some(o.id.clone()) });
                }
            }
        });
    }
    if let Some(a) = answer {
        panel.answer(a);
    }
    ui.add_space(6.0);
    let mut send = false;
    let mut stop = false;
    // The composer: one rounded box holding the text and the Send button, like a chat app.
    egui::Frame::NONE.fill(t.chrome_dark).stroke(egui::Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(8)).show(
        ui,
        |ui| {
            let edit = egui::TextEdit::multiline(&mut panel.input)
                .hint_text(tr("Message Redrob Code"))
                .frame(egui::Frame::NONE)
                .desired_rows(2)
                .desired_width(ui.available_width());
            let r = ui.add_enabled(!panel.busy, edit);
            if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
                send = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if panel.busy {
                    if ui.add_sized(vec2(60.0, 22.0), egui::Button::new(tr("Stop"))).clicked() {
                        stop = true;
                    }
                    ui.spinner();
                } else if ui.add_enabled(!panel.input.trim().is_empty(), egui::Button::new(tr("Send")).min_size(vec2(60.0, 22.0))).clicked() {
                    send = true;
                }
            });
        },
    );
    if send {
        // A trailing newline from Enter is not part of the message.
        let trimmed = panel.input.trim_end_matches('\n').to_string();
        panel.input = trimmed;
        panel.send(&mut app.services.agent);
    }
    if stop {
        panel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_merge_and_tools_update_in_place() {
        let mut p = AgentPanel { busy: true, ..Default::default() };
        p.apply(AgentEvent::Text("Drawing ".into()));
        p.apply(AgentEvent::Text("a circle.".into()));
        p.apply(AgentEvent::Tool { id: "t1".into(), title: "command_line".into(), status: "pending".into() });
        p.apply(AgentEvent::Tool { id: "t1".into(), title: String::new(), status: "completed".into() });
        p.apply(AgentEvent::Text("Done.".into()));
        p.apply(AgentEvent::TurnEnd { stop: "end_turn".into() });
        assert_eq!(
            p.entries,
            vec![
                Entry::Agent("Drawing a circle.".into()),
                Entry::Tool { id: "t1".into(), title: "command_line".into(), status: "completed".into() },
                Entry::Agent("Done.".into()),
            ]
        );
        assert!(!p.busy);
    }

    #[test]
    fn cancelled_turn_leaves_a_notice_and_clears_a_pending_question() {
        let mut p = AgentPanel { busy: true, ..Default::default() };
        p.apply(AgentEvent::Permission { request: 3, title: "execute".into(), options: vec![] });
        assert!(p.pending.is_some());
        p.apply(AgentEvent::TurnEnd { stop: "cancelled".into() });
        assert!(p.pending.is_none());
        assert_eq!(p.entries, vec![Entry::Notice("Stopped.".into())]);
    }

    struct Echo;
    impl AgentHost for Echo {
        fn prompt(&mut self, _: &str) {}
        fn cancel(&mut self) {}
        fn answer(&mut self, _: u64, _: Option<&str>) {}
        fn poll(&mut self) -> Vec<AgentEvent> {
            Vec::new()
        }
    }

    #[test]
    fn a_new_workspace_key_restarts_the_agent() {
        let mut p = AgentPanel { input: "hi".into(), ..Default::default() };
        let mut factory: Option<AgentFactory> = Some(Box::new(|| Ok(Box::new(Echo) as Box<dyn AgentHost>)));
        p.send(&mut factory);
        assert!(p.host.is_some());
        p.account_changed(0);
        assert!(p.host.is_some(), "same generation keeps the agent");
        p.account_changed(1);
        assert!(p.host.is_none() && !p.busy);
        assert_eq!(p.entries.last(), Some(&Entry::Notice("Workspace changed. The next message starts the agent with it.".into())));
    }

    #[test]
    fn without_a_host_factory_the_panel_says_so() {
        let mut p = AgentPanel { input: "draw a box".into(), ..Default::default() };
        p.send(&mut None);
        assert_eq!(p.entries, vec![Entry::Error("The agent runs in the desktop app only.".into())]);
        assert!(!p.busy);
    }
}
