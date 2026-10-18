use std::collections::HashMap;

use analysis_template::{
    base::cheat_analyser_base::CheatAnalyser,
    lib::{
        algorithm::{Detection, analyse, get_algorithms},
        parameters::{Config, SerializableParameter, SerializableParameters, try_update_algorithm_params},
    },
};
use eframe::egui;
use itertools::Itertools;
use steamid_ng::SteamID;
use tf_demo_parser::Demo;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        // .with_file(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 800.0]).with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("Demo Analysis", options, Box::new(|_cc| Ok(Box::new(Gui::new()))))
}

struct Gui {
    algos: HashMap<String, bool>,
    config: Config,
    file: Option<std::path::PathBuf>,
    processing: bool,
    detections: HashMap<SteamID, Vec<Detection>>,
    selected_player: Option<SteamID>,
    selected_detection: Option<usize>,

    analyser: Option<CheatAnalyser>,

    recv: std::sync::mpsc::Receiver<anyhow::Result<CheatAnalyser>>,
    send: std::sync::mpsc::Sender<anyhow::Result<CheatAnalyser>>,
}

impl Gui {
    pub fn new() -> Self {
        let mut config = Config::new();
        for mut a in get_algorithms().drain(..) {
            if let Some(algo_params) = a.params() {
                let serializable_params: SerializableParameters = algo_params.into_iter().map(|(name, param)| (name, param.into())).collect();
                config.insert(a.algorithm_name().to_string(), serializable_params);
            }
        }
        let (send, recv) = std::sync::mpsc::channel();
        let mut gui = Self {
            algos: HashMap::from_iter(get_algorithms().iter().map(|a| (a.algorithm_name().to_string(), a.enabled_by_default()))),
            config,
            file: None,
            processing: false,
            detections: HashMap::new(),
            selected_player: None,
            selected_detection: None,
            analyser: None,
            recv,
            send,
        };
        if let Ok(loaded_config_str) = std::fs::read_to_string("params.json")
            && let Ok(loaded_config) = serde_json::from_str::<Config>(&loaded_config_str)
        {
            gui.update_config(loaded_config);
        }
        gui
    }

    fn analyse(&mut self) {
        let Some(file) = self.file.as_ref() else {
            return;
        };
        self.selected_detection = None;
        self.selected_player = None;
        let mut algorithms = get_algorithms();
        algorithms.retain(|a| self.algos[a.algorithm_name()]);

        for a in algorithms.iter_mut() {
            let algo_name = a.algorithm_name();
            if let (Some(config_params), Some(mut algo_params)) = (self.config.get(a.algorithm_name()), a.params()) {
                tracing::info!("updating params for {}", algo_name);
                try_update_algorithm_params(&mut algo_params, config_params);
            }
        }

        let file = file.to_owned();
        let send = self.send.clone();

        std::thread::spawn(move || {
            match send.send((|| -> anyhow::Result<CheatAnalyser> {
                let file = std::fs::read(&file)?;
                let demo: Demo = Demo::new(&file);
                analyse(&demo, algorithms)
            })()) {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("error while parsing file {:?}: {:?}", file, e);
                }
            };
        });
        self.processing = true;
    }

    // tries to update the internal config based on a config struct
    //
    // use this to update the internal config with the config read from a file
    fn update_config(&mut self, params: Config) {
        for (algo_name, update_params) in params {
            let algo_params_to_update = self.config.entry(algo_name.clone()).or_default();
            for (param_name, param_value) in update_params {
                algo_params_to_update.insert(param_name, param_value);
            }
        }
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
            egui::Window::new("Hover")
                .movable(false)
                .title_bar(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Drop to analyze");
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.processing {
                ui.disable();
                if let Ok(result) = self.recv.try_recv() {
                    let analyser = match result {
                        Ok(x) => x,
                        Err(e) => {
                            tracing::error!("Error while parsing demo: {e:#?}");
                            self.analyser = None;
                            return;
                        }
                    };
                    self.processing = false;
                    self.detections.clear();
                    for det in analyser.detections.clone() {
                        self.detections.entry(det.player).or_default().push(det);
                    }
                    analyser.print_detection_summary();
                    self.analyser = Some(analyser);
                }
            }
            ui.horizontal(|ui| {
                let mut algo_to_configure = None;
                ui.vertical(|ui| {
                    ui.heading("Algorithms");
                    for algo in self.algos.iter_mut().sorted_by_key(|a| a.0) {
                        ui.horizontal(|ui| {
                            ui.checkbox(algo.1, algo.0);
                            if *algo.1 && self.config.get(algo.0).is_some_and(|p| !p.is_empty()) && ui.small_button("⚙").clicked() {
                                algo_to_configure = Some(algo.0.clone());
                            }
                        });
                    }
                });
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                egui::ScrollArea::vertical().min_scrolled_height(300.0).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading("Parameters");
                            if ui.button("Save").clicked() {
                                std::fs::write(
                                    "params.json",
                                    serde_json::to_vec_pretty(&self.config).expect("could not convert config to json"), // TODO: replace expect with error logging
                                )
                                .expect("could not write config to 'params.json'"); // TODO: replace expect with error logging
                            }
                        });
                        ui.separator();
                        for (name, params) in self.config.iter_mut().sorted_by_key(|a| a.0) {
                            if !self.algos[name] {
                                continue;
                            }
                            ui.add_space(10.0);
                            let h = ui.heading(name);
                            if algo_to_configure.as_ref().is_some_and(|n| *n == *name) {
                                h.scroll_to_me(Some(egui::Align::TOP));
                            }
                            ui.separator();
                            // TODO: don't sort every frame
                            for (param_name, param_value) in params.iter_mut().sorted_by_key(|p| p.0) {
                                ui.horizontal(|ui| {
                                    match param_value {
                                        SerializableParameter::Float32(f) => {
                                            ui.add(egui::DragValue::new(f).speed(0.001).max_decimals(50));
                                        }
                                        SerializableParameter::Float64(f) => {
                                            ui.add(egui::DragValue::new(f).speed(0.001).max_decimals(50));
                                        }
                                        SerializableParameter::Bool(b) => {
                                            ui.checkbox(b, "");
                                        }
                                        SerializableParameter::Int32(v) => {
                                            ui.add(egui::DragValue::new(v).speed(1));
                                        }
                                        SerializableParameter::Int64(v) => {
                                            ui.add(egui::DragValue::new(v).speed(1));
                                        }
                                        SerializableParameter::Usize(v) => {
                                            ui.add(egui::DragValue::new(v).speed(1));
                                        }
                                        SerializableParameter::Unsigned8(v) => {
                                            ui.add(egui::DragValue::new(v).speed(1));
                                        }
                                        SerializableParameter::Unsigned32(v) => {
                                            ui.add(egui::DragValue::new(v).speed(1));
                                        }
                                        SerializableParameter::Unsigned64(v) => {
                                            ui.add(egui::DragValue::new(v).speed(1));
                                        }
                                    }
                                    ui.label(param_name);
                                });
                            }
                        }
                    });
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Open...").clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter("Demos", &["dem"]).pick_file()
                {
                    self.file = Some(path);
                }
                ui.add_enabled_ui(self.file.is_some(), |ui| {
                    if ui.button("Analyse").clicked() {
                        self.analyse();
                    }
                    if ui.button("Save detections").clicked()
                        && let Some(a) = &self.analyser
                        && let Some(path) = rfd::FileDialog::new().set_file_name("detections.json").save_file()
                    {
                        let analysis = serde_json::json!({
                            "server_ip": a.header.as_ref().map_or("unknown".to_string(), |h| h.server.clone()),
                            "duration": a.tick,
                            "author": a.header.as_ref().map_or("unknown".to_string(), |h| h.nick.clone()),
                            "map": a.header.as_ref().map_or("unknown".to_string(), |h| h.map.clone()),
                            "detections": a.detections
                        });
                        match serde_json::to_vec_pretty(&analysis) {
                            Ok(value) => match std::fs::write(&path, &value) {
                                Ok(_) => tracing::info!("successfully saved detections to file"),
                                Err(e) => tracing::error!("could not save detections to file ({path:?}): {e:#?}"),
                            },
                            Err(e) => tracing::error!("could not convert detections to json: {e:#?}"),
                        }
                    }
                });
            });
            if self.processing {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Analysing...");
                    let progress = analysis_template::PROGRESS_CURRENT.load(std::sync::atomic::Ordering::Relaxed);
                    let total = analysis_template::PROGRESS_TOTAL.load(std::sync::atomic::Ordering::Relaxed);
                    ui.add(
                        egui::widgets::ProgressBar::new(progress as f32 / total as f32)
                            .show_percentage()
                            .text(format!("{} / {}", progress, total)),
                    );
                });
            }
            ui.add_space(10.0);
            if let Some(p) = &self.file {
                ui.heading(p.file_name().expect("only valid files should be selectable").to_string_lossy());
                ui.label("Doubleclick steamid to open profile");
            }
            ui.separator();
            ui.horizontal_top(|ui| {
                egui::ScrollArea::vertical().id_salt("players").show(ui, |ui| {
                    ui.set_min_width(160.0);
                    ui.vertical(|ui| {
                        for (&steam_id, detections) in self.detections.iter().sorted_by_key(|d| d.1.len()).rev() {
                            let res = ui.selectable_label(
                                self.selected_player.is_some_and(|u| u == steam_id),
                                format!("{}/{} ({})", u64::from(steam_id), steam_id.steam3(), detections.len()),
                            );
                            if res.clicked() {
                                self.selected_player = Some(steam_id);
                                self.selected_detection = None;
                            }
                            if res.double_clicked() {
                                let _ = opener::open_browser(format!("https://steamcommunity.com/profiles/{}", u64::from(steam_id)));
                            }
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("detections").show(ui, |ui| {
                    ui.set_min_width(160.0);
                    ui.vertical(|ui| {
                        if let Some(detections) = self.selected_player.and_then(|p| self.detections.get(&p)) {
                            for (i, det) in detections.iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_detection.is_some_and(|si| si == i), format!("{}: {}", det.tick, det.algorithm))
                                    .clicked()
                                {
                                    self.selected_detection = Some(i);
                                }
                            }
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("details").show(ui, |ui| {
                    if let Some(det) = self
                        .selected_player
                        .and_then(|p| self.detections.get(&p))
                        .and_then(|dets| self.selected_detection.and_then(|di| dets.get(di)))
                    {
                        ui.label(serde_json::to_string_pretty(&det.data).expect("detection data should be valid json"));
                    }
                });
            });
        });
        if let Some(f) = ctx.input(|i| i.raw.dropped_files.first().cloned()) {
            self.file = f.path;
            self.analyse();
        }
    }
}
