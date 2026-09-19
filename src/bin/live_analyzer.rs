use analysis_template::{
    PRINT_VOTES, SILENT, dev_print,
    lib::{
        algorithm::{AimSnap, AngleHistory, AngleRepeat, BackTrack, CheatAlgorithm, DoubleTap, OOBPitch, ViewAngles180Degrees},
        parameters::Parameters,
    },
    live_analyzer::live_analysis,
};
use anyhow::Context;
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};
use steamlocate::SteamDir;
use tracing::Level;

fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        // .with_file(true)
        .with_max_level(Level::INFO)
        .init();

    let demo_directory = {
        let mut p = locate_tf2_folder().expect("could not locate tf2 directory");
        p.push("tf");
        p.push("demos");
        p
    };

    // println!("demo_path: {:?}", demo_path);
    tracing::info!("demo directory: {:?}", demo_directory);

    // initialize the config and show changes
    _ = get_desired_algorithms();

    PRINT_VOTES.store(true, std::sync::atomic::Ordering::Relaxed);
    SILENT.store(true, std::sync::atomic::Ordering::Relaxed);
    let result = live_analysis(demo_directory, get_desired_algorithms);
    match result {
        Ok(_analysers) => {
            // TODO: maybe display some stats about the analysers
            tracing::info!("finished live analysis");
        }
        Err(e) => {
            tracing::error!("live_analysis terminated with {:?}", e);
        }
    }
}

const TF2_GAME_ID: u32 = 440;

fn locate_tf2_folder() -> anyhow::Result<PathBuf> {
    let (app, library) = SteamDir::locate()
        .context("Failed to locate Steam directory")?
        .find_app(TF2_GAME_ID)
        .context("Failed to locate TF2 installation.")?
        .ok_or_else(|| anyhow::anyhow!("Failed to locate TF2 installation."))?;
    Ok(library.resolve_app_dir(&app))
}

fn get_desired_algorithms() -> Vec<Box<dyn CheatAlgorithm + Send>> {
    // let mut ah = AngleHistory::default();
    // if let Some(params) = ah.params() {
    //     *params = HashMap::from([
    //         ("tick_window".to_string(), Parameter::Int(4)),
    //         ("max_delta_first_third".to_string(), Parameter::Float(0.45)),
    //         ("min_delta_second_third".to_string(), Parameter::Float(4.5)),
    //     ]);
    // }

    let mut algos: Vec<Box<dyn CheatAlgorithm + Send>> = vec![
        // Box::new(ViewAngles180Degrees::new()),
        Box::new(OOBPitch::new()),
        Box::new(AngleRepeat::new()),
        Box::new(AngleHistory::default()),
        Box::new(AimSnap::new()),
        Box::new(BackTrack::new()),
        Box::new(DoubleTap::new()),
        // Box::new(AngleAccelerationAnalyzer::default()),
    ];
    config_algorithms_desired(&mut algos);
    algos
}

fn config_algorithms_desired(algorithms: &mut Vec<Box<dyn CheatAlgorithm + Send>>) {
    let Some(config) = CONFIG.as_ref() else { return };

    for algo in algorithms.iter_mut() {
        let algorithm_name = algo.algorithm_name().to_owned();

        let Some(algo_params) = algo.params() else {
            continue;
        };

        dev_print!("  {}", algorithm_name);
        algo_params.iter_mut().for_each(|(k, v)| {
            if let Some(config_params) = config.get(&algorithm_name) {
                if let Some(param_value) = config_params.get(k) {
                    *v = param_value.clone();
                    dev_print!("    {} = {:?} (changed)", k, v);
                } else {
                    dev_print!("    {} = {:?} (default)", k, v);
                }
            } else {
                dev_print!("    {} = {:?} (default)", k, v);
            }
        });
    }
}

// TODO: check if this needs to be thread safe
static CONFIG: LazyLock<Option<Config>> = LazyLock::new(get_config_json);
type Config = HashMap<String, Parameters>;

fn get_config_json() -> Option<Config> {
    tracing::info!("initializing config");
    let path = std::env::args().nth(1)?;
    let file_content = std::fs::read(&path).unwrap_or_else(|_| panic!("could not read file at {}", path));

    serde_json::from_slice(&file_content).unwrap_or_else(|_| panic!("could not parse file at {path}"))
}
