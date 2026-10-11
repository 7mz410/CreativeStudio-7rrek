//! Creative Studio: Help › AI Agents. AI agents (Claude, Gemini and other MCP clients) drive the app
//! through the localhost control channel and `vectorcraft-cli mcp`. This window turns the channel
//! on (it then also starts with the app) and adds the app's MCP server to an agent's settings.
//! The desktop app provides the work through [`AgentsService`]; without it (the web) the window
//! says agents need the desktop app.

use egui::{Context, RichText};

use crate::theme::Tokens;
use crate::{VectorcraftApp, widgets};

/// An agent app whose settings can list our MCP server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentApp {
    ClaudeDesktop,
    ClaudeCode,
    GeminiCli,
}

impl AgentApp {
    pub const ALL: [AgentApp; 3] = [AgentApp::ClaudeDesktop, AgentApp::ClaudeCode, AgentApp::GeminiCli];

    pub fn label(self) -> &'static str {
        match self {
            AgentApp::ClaudeDesktop => "Claude Desktop",
            AgentApp::ClaudeCode => "Claude Code",
            AgentApp::GeminiCli => "Gemini CLI",
        }
    }
}

/// What the desktop app does for this window.
pub trait AgentsService {
    /// Is the control channel allowed (it then starts with the app)?
    fn enabled(&self) -> bool;
    /// Allow or stop agents: saves the choice and starts the channel now when allowing it.
    /// Returns the requests of a channel it started.
    fn set_enabled(&mut self, on: bool) -> Result<Option<std::sync::mpsc::Receiver<crate::control::ControlRequest>>, String>;
    /// Is our MCP server in this agent app's settings?
    fn connected_to(&self, app: AgentApp) -> bool;
    /// Add our MCP server to this agent app's settings.
    fn connect(&mut self, app: AgentApp) -> Result<(), String>;
    /// The settings snippet for other MCP clients.
    fn snippet(&self) -> String;
}

fn open_id() -> egui::Id {
    egui::Id::new("studio_agents_open")
}

/// Help › AI Agents… asked for the window (commands have no egui context; the next frame opens it).
static OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Help › AI Agents…
pub fn open() {
    OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The window, while it is open.
pub fn show(app: &mut VectorcraftApp, ctx: &Context) {
    let asked = OPEN.swap(false, std::sync::atomic::Ordering::Relaxed);
    let mut open = asked || ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false);
    if !open {
        return;
    }
    let t = Tokens::get(ctx);
    let mut started = None;
    let mut error: Option<String> = ctx.data(|d| d.get_temp(egui::Id::new("studio_agents_error")));
    egui::Window::new(tl!("AI Agents"))
        .id(egui::Id::new("studio_agents"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.set_width(420.0);
            // One short line each: wrapped Arabic lines would show in the wrong order (bidi.rs).
            ui.label(tl!("AI agents such as Claude and Gemini can work in this app while you watch."));
            ui.label(tl!("They connect on this computer only."));
            ui.add_space(10.0);
            let Some(svc) = app.services.agents.as_mut() else {
                ui.label(RichText::new(tl!("Agents need the desktop app.")).color(t.text_dim));
                return;
            };
            let on = !svc.enabled();
            if widgets::check(ui, tl!("Allow AI agents to control this app"), !on, true) {
                match svc.set_enabled(on) {
                    Ok(rx) => {
                        started = rx;
                        error = None;
                    }
                    Err(e) => error = Some(e),
                }
            }
            if svc.enabled() {
                ui.label(RichText::new(tl!("Agents can connect now.")).color(t.accent));
            }
            ui.add_space(12.0);
            ui.label(RichText::new(tl!("Connect an agent app")).strong());
            ui.label(RichText::new(tl!("Adds this app to the agent's settings.")).color(t.text_dim));
            ui.label(RichText::new(tl!("Restart the agent app afterwards.")).color(t.text_dim));
            ui.add_space(4.0);
            for a in AgentApp::ALL {
                ui.horizontal(|ui| {
                    ui.label(a.label());
                    if svc.connected_to(a) {
                        ui.label(RichText::new(tl!("Connected")).color(t.accent));
                    } else if ui.button(tl!("Connect")).clicked() {
                        error = svc.connect(a).err();
                    }
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new(tl!("Other agents")).strong());
            ui.label(RichText::new(tl!("Add this to the agent's MCP settings:")).color(t.text_dim));
            let mut s = svc.snippet();
            ui.add(egui::TextEdit::multiline(&mut s).code_editor().desired_rows(4).desired_width(f32::INFINITY));
            if let Some(e) = &error {
                ui.add_space(6.0);
                ui.label(RichText::new(e).color(t.error));
            }
        });
    ctx.data_mut(|d| {
        d.insert_temp(open_id(), open);
        match &error {
            Some(e) => {
                d.insert_temp(egui::Id::new("studio_agents_error"), e.clone());
            }
            None => {
                d.remove::<String>(egui::Id::new("studio_agents_error"));
            }
        }
    });
    if let Some(rx) = started {
        app.set_control(rx);
    }
}
