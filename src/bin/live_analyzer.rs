use analysis_template::{
    PRINT_VOTES,
    SILENT,
    // algorithms::tex::angle_acceleration::AngleAccelerationAnalyzer,
    lib::algorithm::{AimSnap, AngleHistory, AngleRepeat, BackTrack, CheatAlgorithm, DoubleTap, OOBPitch, ViewAngles180Degrees},
    live_analyzer::live_analysis,
};
use anyhow::Context;
use std::{env::args, path::PathBuf};
use steamlocate::SteamDir;
use tracing::Level;

fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        // .with_file(true)
        .with_max_level(Level::INFO)
        .init();

    SILENT.store(true, std::sync::atomic::Ordering::Relaxed);
    PRINT_VOTES.store(true, std::sync::atomic::Ordering::Relaxed);

    let demo_path = if let Some(path) = args().nth(1) {
        path.into()
    } else {
        let mut p = locate_tf2_folder().expect("could not locate tf2 directory");
        p.push("tf");
        p.push("demos");
        p
    };

    // println!("demo_path: {:?}", demo_path);
    tracing::info!("demo_path: {:?}", demo_path);

    let result = live_analysis(demo_path, get_desired_algorithms);
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
    vec![
        Box::new(ViewAngles180Degrees::new()),
        Box::new(OOBPitch::new()),
        Box::new(AngleRepeat::new()),
        Box::new(AngleHistory::default()),
        Box::new(AimSnap::new()),
        Box::new(BackTrack::new()),
        Box::new(DoubleTap::new()),
        // Box::new(AngleAccelerationAnalyzer::default()),
    ]
}
