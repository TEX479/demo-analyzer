use std::{
    env::args,
    path::{Path, PathBuf},
    time::Instant,
};

use analysis_template::{
    SILENT,
    base::cheat_analyser_base::CheatAnalyser,
    lib::algorithm::{AimSnap, AngleHistory, AngleRepeat, BackTrack, CheatAlgorithm, DoubleTap, OOBPitch, ViewAngles180Degrees, analyse},
};
use anyhow::Context;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use steamlocate::SteamDir;
use tf_demo_parser::Demo;
use tracing::Level;

fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        // .with_file(true)
        .with_max_level(Level::INFO)
        .init();

    SILENT.store(true, std::sync::atomic::Ordering::Relaxed);
    // PRINT_VOTES.store(true, std::sync::atomic::Ordering::Relaxed);

    let demo_path = if let Some(path) = args().nth(1) {
        path.into()
    } else {
        let mut p = locate_tf2_folder().expect("could not locate tf2 directory");
        p.push("tf");
        p.push("demos");
        p
    };

    tracing::info!("demos path: {:?}", demo_path);

    let files = get_demos_in_dir(&demo_path).expect("path must be readable");

    let files_len = files.len();

    let start = Instant::now();
    let analysed = files
        .into_par_iter()
        .map(|path| analyze_demo_at_path(&path).context(format!("while analysing {:?}", path)))
        .collect::<Vec<_>>();
    let elapsed = start.elapsed();
    tracing::info!("finished analysing {} files in {:.02}s", files_len, elapsed.as_secs_f64());

    SILENT.store(false, std::sync::atomic::Ordering::Relaxed);

    for analyzer_result in analysed.into_iter() {
        match analyzer_result {
            Ok(a) => {
                if !a.detections.is_empty() {
                    a.print_metadata();
                    a.print_detection_summary();
                }
            }
            Err(e) => {
                tracing::error!("{:?}", e);
            }
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

fn get_demos_in_dir(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    Ok(std::fs::read_dir(dir)
        .context(format!("Could not read folder {:?}", dir))?
        .filter_map(|x| x.ok())
        .map(|x| x.path())
        .filter_map(|p| {
            if p.is_file() {
                Some(vec![p])
            } else if p.is_dir() {
                get_demos_in_dir(&p).ok()
            } else {
                None
            }
        })
        .flatten()
        .filter(|file| file.extension().is_some_and(|e| e == "dem"))
        .collect::<Vec<_>>())
}

fn analyze_demo_at_path(path: &Path) -> anyhow::Result<CheatAnalyser> {
    let file = std::fs::read(path)?;
    let demo = Demo::new(&file);
    analyse(&demo, desired_algorithms())
}

fn desired_algorithms() -> Vec<Box<dyn CheatAlgorithm + Send>> {
    vec![
        Box::new(ViewAngles180Degrees::new()),
        Box::new(OOBPitch::new()),
        Box::new(AngleRepeat::new()),
        Box::new(AngleHistory::default()),
        Box::new(AimSnap::new()),
        Box::new(BackTrack::new()),
        Box::new(DoubleTap::new()),
    ]
}
