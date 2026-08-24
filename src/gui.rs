use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::{gateway, run_scan};

pub fn run() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MicroModem")
            .with_inner_size([940.0, 720.0])
            .with_min_inner_size([680.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MicroModem",
        options,
        Box::new(|cc| {
            configure_style(&cc.egui_ctx);
            Ok(Box::new(MicroModemApp::new()))
        }),
    )
    .expect("could not start the MicroModem GUI");
}

fn configure_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(18, 17, 22);
    visuals.window_fill = Color32::from_rgb(32, 30, 38);
    visuals.extreme_bg_color = Color32::from_rgb(21, 19, 25);
    visuals.selection.bg_fill = Color32::from_rgb(86, 68, 139);
    visuals.widgets.active.bg_fill = Color32::from_rgb(143, 117, 230);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(72, 61, 91);
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(13.0, 8.0);
    ctx.set_style(style);
}

struct MicroModemApp {
    socks5_addr: String,
    socks5_port: String,
    downstream_if: String,
    mode: String,
    ssid: String,
    passphrase: String,
    channel: String,
    country: String,
    notice: String,
    status: String,
    interfaces: Vec<(String, String, String)>,
    proxies: Vec<(String, String, bool)>,
    logs: Option<String>,
}

impl MicroModemApp {
    fn new() -> Self {
        let c = gateway::parse_config();
        let mut app = Self {
            socks5_addr: c.get("SOCKS5_ADDR").cloned().unwrap_or_default(),
            socks5_port: c.get("SOCKS5_PORT").cloned().unwrap_or_else(|| "8228".into()),
            downstream_if: c.get("DOWNSTREAM_IF").cloned().unwrap_or_default(),
            mode: c.get("DOWNSTREAM_MODE").cloned().unwrap_or_else(|| "wifi".into()),
            ssid: c.get("WIFI_SSID").cloned().unwrap_or_else(|| "MicroModem".into()),
            passphrase: c.get("WIFI_PASSPHRASE").cloned().unwrap_or_default(),
            channel: c.get("WIFI_CHANNEL").cloned().unwrap_or_else(|| "6".into()),
            country: c.get("WIFI_COUNTRY").cloned().unwrap_or_else(|| "US".into()),
            notice: "Ready. Scan, configure, then start the gateway.".into(),
            status: String::new(),
            interfaces: vec![],
            proxies: vec![],
            logs: None,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.status = gateway::gateway_status().unwrap_or_else(|e| e);
    }

    fn save(&mut self) -> bool {
        match gateway::save_gateway_config(
            self.socks5_addr.trim().into(), self.socks5_port.clone(),
            self.downstream_if.trim().into(), self.mode.clone(), self.ssid.clone(),
            self.passphrase.clone(), self.channel.clone(), self.country.trim().into(),
        ) {
            Ok(message) => { self.notice = message; true }
            Err(error) => { self.notice = error; false }
        }
    }

    fn action(&mut self, action: &str) {
        if action != "stop" && !self.save() { return; }
        self.notice = format!("{}ing gateway… complete the administrator prompt.", action);
        self.notice = gateway::gateway_action(action.into()).unwrap_or_else(|e| e);
        self.refresh();
    }

    fn scan(&mut self) {
        self.notice = "Scanning candidate networks and proxy protocols…".into();
        let (interfaces, findings) = run_scan(None, None, Duration::from_millis(700), false);
        self.interfaces = interfaces.into_iter().map(|i| (
            i.name,
            i.addresses.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "),
            i.gateway.map(|g| g.to_string()).unwrap_or_default(),
        )).collect();
        self.proxies = findings.into_iter().map(|f| (
            f.host.to_string(), f.port.to_string(),
            matches!(f.kind, crate::Kind::Socks5 { udp_associate: true }),
        )).collect();
        self.notice = "Network scan complete.".into();
    }
}

impl eframe::App for MicroModemApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("ANDROID CELLULAR GATEWAY").color(Color32::from_rgb(200,182,255)).strong());
                        ui.heading(RichText::new("MicroModem").size(38.0));
                        ui.label("Turn a phone-hosted proxy into Ethernet or Wi-Fi.");
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let running = self.status.contains("micromodem-tunnel") && !self.status.contains("not running");
                        ui.label(if running { "● Gateway running" } else { "○ Gateway stopped" });
                    });
                });
                ui.add_space(12.0);
                egui::Frame::group(ui.style()).inner_margin(14).show(ui, |ui| { ui.label(&self.notice); });

                section(ui, "01 · UPLINK", "Find the phone", |ui| {
                    if ui.button("Scan networks").clicked() { self.scan(); }
                    ui.columns(2, |cols| {
                        cols[0].heading("Interfaces");
                        if self.interfaces.is_empty() { cols[0].label("No scan has run yet."); }
                        for (name, addresses, gateway) in self.interfaces.clone() {
                            if cols[0].button(format!("{name}\n{addresses}\ngateway {gateway}")).clicked() { self.downstream_if = name; }
                        }
                        cols[1].heading("Proxy endpoints");
                        if self.proxies.is_empty() { cols[1].label("No scan has run yet."); }
                        for (host, port, udp) in self.proxies.clone() {
                            if cols[1].button(format!("SOCKS5\n{host}:{port}\nUDP {}", if udp { "accepted" } else { "unavailable" })).clicked() {
                                self.socks5_addr = host; self.socks5_port = port;
                            }
                        }
                    });
                });

                section(ui, "02 · CONFIGURE", "Gateway", |ui| {
                    egui::Grid::new("config").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                        field(ui, "SOCKS5 address", &mut self.socks5_addr); field(ui, "Port", &mut self.socks5_port); ui.end_row();
                        field(ui, "Output interface", &mut self.downstream_if);
                        ui.vertical(|ui| { ui.label("Output mode"); egui::ComboBox::from_id_salt("mode").selected_text(&self.mode).show_ui(ui, |ui| { ui.selectable_value(&mut self.mode, "wifi".into(), "Wi-Fi access point"); ui.selectable_value(&mut self.mode, "ethernet".into(), "Ethernet handoff"); }); }); ui.end_row();
                        if self.mode == "wifi" {
                            field(ui, "Network name", &mut self.ssid); password(ui, "Password", &mut self.passphrase); ui.end_row();
                            field(ui, "Channel", &mut self.channel); field(ui, "Country", &mut self.country); ui.end_row();
                        }
                    });
                    if ui.button("Save configuration").clicked() { self.save(); }
                });

                section(ui, "03 · ROUTE", "Controls", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Start gateway").clicked() { self.action("start"); }
                        if ui.button("Restart").clicked() { self.action("restart"); }
                        if ui.button("Stop").clicked() { self.action("stop"); }
                        if ui.button("Refresh").clicked() { self.refresh(); }
                        if ui.button("View logs").clicked() { self.logs = Some(gateway::gateway_logs().unwrap_or_else(|e| e)); }
                    });
                    egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| { ui.monospace(&self.status); });
                });
                ui.label(RichText::new("Routed TCP/UDP over SOCKS5 · RNDIS uplink · nftables isolation").weak());
            });
        });
        if let Some(logs) = &mut self.logs {
            let mut open = true;
            egui::Window::new("Gateway logs").open(&mut open).resizable(true).show(ctx, |ui| { egui::ScrollArea::vertical().show(ui, |ui| { ui.monospace(logs.as_str()); }); });
            if !open { self.logs = None; }
        }
    }
}

fn section(ui: &mut egui::Ui, step: &str, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(12.0);
    egui::Frame::group(ui.style()).inner_margin(18).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(step).color(Color32::from_rgb(200,182,255)).strong());
        ui.heading(title);
        ui.separator();
        body(ui);
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) { ui.vertical(|ui| { ui.label(label); ui.text_edit_singleline(value); }); }
fn password(ui: &mut egui::Ui, label: &str, value: &mut String) { ui.vertical(|ui| { ui.label(label); ui.add(egui::TextEdit::singleline(value).password(true)); }); }
