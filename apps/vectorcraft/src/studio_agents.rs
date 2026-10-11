//! Creative Studio: Help › AI Agents on the desktop. The choice to allow agents is saved next to the
//! preferences (`studio-agents.json`); when allowed, the localhost control channel starts with
//! the app (or at once). Agent apps reach it through `vectorcraft-cli mcp`, which this module adds
//! to their MCP settings.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use serde_json::{Value, json};
use vectorcraft_ui_egui::ControlRequest;
use vectorcraft_ui_egui::agents::{AgentApp, AgentsService};

/// The port agents connect to (`vectorcraft-cli mcp` tries it first).
pub const PORT: u16 = 7979;
/// Our entry in an agent app's `mcpServers`.
const SERVER_NAME: &str = "studio-rsm";

pub struct Agents {
    settings: Option<PathBuf>,
    ctx: egui::Context,
    /// Requests are passed on to the app only while this is on.
    on: Arc<AtomicBool>,
    started: bool,
}

impl Agents {
    pub fn new(settings_dir: Option<PathBuf>, ctx: egui::Context) -> Self {
        let settings = settings_dir.map(|d| d.join("studio-agents.json"));
        let on = settings
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["enabled"].as_bool())
            .unwrap_or(false);
        Self { settings, ctx, on: Arc::new(AtomicBool::new(on)), started: false }
    }

    /// The app already serves a control channel (`--control`): don't start another.
    pub fn mark_started(&mut self) {
        self.started = true;
    }

    /// Start the channel if agents are allowed and it isn't running yet: the app's requests.
    pub fn start_if_enabled(&mut self) -> Option<Receiver<ControlRequest>> {
        if !self.on.load(Ordering::Relaxed) || self.started {
            return None;
        }
        self.started = true;
        let from_server = crate::control_server::start(PORT, self.ctx.clone());
        let (to_app, rx) = channel::<ControlRequest>();
        let (on, ctx) = (Arc::clone(&self.on), self.ctx.clone());
        std::thread::spawn(move || {
            for req in from_server {
                if on.load(Ordering::Relaxed) {
                    if to_app.send(req).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                } else {
                    // Best effort: the agent may have gone.
                    let _ = req.reply.send(json!({"ok": false, "error": "AI agents are turned off (Help › AI Agents)"}));
                }
            }
        });
        Some(rx)
    }

    fn cli() -> String {
        let exe = std::env::current_exe().ok();
        let dir = exe.as_deref().and_then(Path::parent);
        let name = if cfg!(windows) { "vectorcraft-cli.exe" } else { "vectorcraft-cli" };
        dir.map(|d| d.join(name)).unwrap_or_else(|| PathBuf::from(name)).to_string_lossy().into_owned()
    }

    fn entry() -> Value {
        json!({"command": Self::cli(), "args": ["mcp"]})
    }

    fn config_path(app: AgentApp) -> Option<PathBuf> {
        let home = PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?);
        Some(match app {
            AgentApp::ClaudeDesktop => {
                if cfg!(target_os = "macos") {
                    home.join("Library/Application Support/Claude/claude_desktop_config.json")
                } else if cfg!(windows) {
                    PathBuf::from(std::env::var_os("APPDATA")?).join("Claude/claude_desktop_config.json")
                } else {
                    home.join(".config/Claude/claude_desktop_config.json")
                }
            }
            AgentApp::ClaudeCode => home.join(".claude.json"),
            AgentApp::GeminiCli => home.join(".gemini/settings.json"),
        })
    }

    fn read_config(path: &Path) -> Result<Value, String> {
        match std::fs::read(path) {
            Ok(b) if !b.is_empty() => serde_json::from_slice(&b).map_err(|e| format!("{} can't be read: {e}", path.display())),
            _ => Ok(json!({})),
        }
    }
}

impl AgentsService for Agents {
    fn enabled(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }

    fn set_enabled(&mut self, on: bool) -> Result<Option<Receiver<ControlRequest>>, String> {
        if let Some(p) = &self.settings {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(p, json!({"enabled": on}).to_string()).map_err(|e| e.to_string())?;
        }
        self.on.store(on, Ordering::Relaxed);
        Ok(self.start_if_enabled())
    }

    fn connected_to(&self, app: AgentApp) -> bool {
        Self::config_path(app).and_then(|p| Self::read_config(&p).ok()).is_some_and(|v| v["mcpServers"][SERVER_NAME].is_object())
    }

    fn connect(&mut self, app: AgentApp) -> Result<(), String> {
        let path = Self::config_path(app).ok_or("no home folder")?;
        let mut v = Self::read_config(&path)?;
        let Some(obj) = v.as_object_mut() else { return Err(format!("{} isn't a settings object", path.display())) };
        // Keep a copy of the agent's settings as they were before the first change.
        let backup = path.with_extension("json.bak-studio");
        if path.exists() && !backup.exists() {
            std::fs::copy(&path, &backup).map_err(|e| e.to_string())?;
        }
        let servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
        let Some(servers) = servers.as_object_mut() else { return Err("mcpServers isn't an object".into()) };
        servers.insert(SERVER_NAME.into(), Self::entry());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())
    }

    fn snippet(&self) -> String {
        serde_json::to_string_pretty(&json!({"mcpServers": {SERVER_NAME: Self::entry()}})).unwrap_or_default()
    }
}
