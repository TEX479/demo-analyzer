// TODO: add config options for:
// - config file (`params.json`)
// - which algos to run
// - tf2 path

use analysis_template::{
    lib::{
        algorithm::{CheatAlgorithm, get_algorithms},
        parameters::{Config, try_update_algorithm_params},
    },
    live_analyzer::live_analysis,
    util::helpers::locate_tf2_folder,
};
use getopts::Options;
use std::path::{Path, PathBuf};
use tracing::Level;

fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        // .with_file(true)
        .with_max_level(Level::INFO)
        .init();

    let mut opts = Options::new();
    opts.optmulti(
        "a",
        "algorithm",
        "specify the algorithm to run. Include multiple -a flags to run multiple algorithms.",
        "[ALGORITHM [-a ALGORITHM]...]",
    );
    opts.optopt("p", "params", "Parameter json file to use for the algorithms", "PATH");
    opts.optopt("d", "demo-path", "Path to the folder to watch for demo files", "PATH");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(std::env::args().skip(1)) {
        Ok(m) => m,
        Err(_) => {
            print_help(&opts);
            return;
        }
    };

    let Some(demo_directory) = matches.opt_str("d").map(PathBuf::from).or_else(|| {
        locate_tf2_folder().ok().map(|mut p| {
            p.push("tf");
            p.push("demos");
            p
        })
    }) else {
        tracing::error!("Path to \"demos\" directory not set");
        return;
    };

    if matches.opt_present("h") {
        print_help(&opts);
        return;
    }

    let config_path = matches.opt_str("p").map(PathBuf::from);

    let config = match config_path.as_ref() {
        Some(path) => match get_config_json(path) {
            Some(config) => Some(config),
            None => {
                tracing::warn!("could not load {:?}. using default parameters", path);
                None
            }
        },
        None => None,
    };
    let config_opt = config.as_ref();

    let algorithms_option_value = matches.opt_present("a").then(|| matches.opt_strs("a"));
    let algo_opts = algorithms_option_value.as_ref();

    // println!("demo_path: {:?}", demo_path);
    tracing::info!("demo directory: {:?}", demo_directory);

    let result = live_analysis(demo_directory, || get_and_config_desired_algorithms(algo_opts, config_opt));
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

fn print_help(opts: &getopts::Options) {
    println!("{}", opts.usage("Usage: analysis-template [options]"));
}

fn config_algorithms_desired(algorithms: &mut Vec<Box<dyn CheatAlgorithm + Send>>, config_opt: Option<&Config>) {
    let Some(config) = config_opt else { return };

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

fn get_and_config_desired_algorithms(algos_opt: Option<&Vec<String>>, config_opt: Option<&Config>) -> Vec<Box<dyn CheatAlgorithm + Send>> {
    let mut algos = get_algorithms();

    if let Some(selection) = algos_opt {
        algos.retain(|a| selection.iter().find(|n| a.algorithm_name() == *n).is_some());
    } else {
        algos.retain(|a| a.enabled_by_default());
    }

    config_algorithms_desired(&mut algos, config_opt);
    algos
}

fn get_config_json(path: &Path) -> Option<Config> {
    tracing::info!("initializing config");
    let Ok(file_content) = std::fs::read(path) else {
        tracing::error!("could not read file at {:?}", path);
        return None;
    };

    match serde_json::from_slice(&file_content) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::error!("could not parse file at {path:?}: {e:?}");
            None
        }
    }
}
