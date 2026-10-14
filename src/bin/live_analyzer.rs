use analysis_template::{
    SILENT,
    lib::{
        algorithm::{CheatAlgorithm, get_algorithms},
        parameters::{Config, try_update_algorithm_params},
    },
    live_analyzer::live_analysis,
};
use anyhow::Context;
use std::{path::PathBuf, sync::LazyLock};
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
    let mut algos = get_algorithms().into_iter().filter(|x| x.enabled_by_default()).collect();
    config_algorithms_desired(&mut algos);
    algos
}

fn config_algorithms_desired(algorithms: &mut Vec<Box<dyn CheatAlgorithm + Send>>) {
    let Some(config) = CONFIG.as_ref() else { return };

    for algo in algorithms.iter_mut() {
        let algorithm_name = algo.algorithm_name();

        let Some(mut algo_params) = algo.params() else {
            continue;
        };

        let Some(config_params) = config.get(algorithm_name) else {
            tracing::info!("no config found for {:?}, using default: {:?}", algorithm_name, algo_params);
            continue;
        };

        tracing::info!("updating params for {}", algorithm_name);
        try_update_algorithm_params(&mut algo_params, config_params);
    }
}

static CONFIG: LazyLock<Option<Config>> = LazyLock::new(get_config_json);

fn get_config_json() -> Option<Config> {
    tracing::info!("initializing config");
    let path = std::env::args().nth(1)?;
    let Ok(file_content) = std::fs::read(&path) else {
        tracing::error!("could not read file at {}", path);
        return None;
    };

    match serde_json::from_slice(&file_content) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::error!("could not parse file at {path}: {e:?}");
            None
        }
    }
}
