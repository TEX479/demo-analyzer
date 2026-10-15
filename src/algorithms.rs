// this file exists for my (tex) editor's rust-analyzer.
// it does not lint the files added by `build.rs`, so this is a fix

use crate::lib::algorithm::CheatAlgorithm;

pub mod fidoo {
    pub mod auto_backstab;
    pub mod bunnyhop;
    pub mod invalid_equip_region;
    pub mod nospread;
    pub mod plain_aimbot;
    pub mod psilent5;
    pub mod pyro_aimbot;
    pub mod silent_aim;
}
pub mod nocrex {
    pub mod aimsnap;
    pub mod angle_repeat;
    pub mod oob_pitch;
}
pub mod teltta {
    pub mod angle_history;
    pub mod backtrack;
    pub mod double_tap;
}
pub mod tex {
    pub mod info_logger;
}
pub mod all_messages;
pub mod viewangles_180degrees;
pub mod viewangles_to_csv;
pub mod write_to_file;

pub fn get_proven_algorithms() -> Vec<Box<dyn CheatAlgorithm + Send>> {
    vec![
        Box::new(fidoo::auto_backstab::AutoBackstab::new()),
        Box::new(fidoo::bunnyhop::BunnyHop::new()),
        Box::new(fidoo::invalid_equip_region::InvalidEquipRegion::new()),
        Box::new(fidoo::nospread::NoSpread::new()),
        Box::new(fidoo::plain_aimbot::PlainAimbot::new()),
        Box::new(fidoo::psilent5::Psilent5::new()),
        Box::new(fidoo::pyro_aimbot::PyroAimbot::new()),
        Box::new(fidoo::silent_aim::SilentAim::new()),
        Box::new(nocrex::aimsnap::AimSnap::new()),
        Box::new(nocrex::angle_repeat::AngleRepeat::new()),
        Box::new(nocrex::oob_pitch::OOBPitch::new()),
        Box::new(teltta::angle_history::AngleHistory::new()),
        Box::new(teltta::backtrack::BackTrack::new()),
        Box::new(teltta::double_tap::DoubleTap::new()),
        Box::new(tex::info_logger::Logger::new()),
        // Box::new(all_messages::AllMessages::new()),
        // Box::new(viewangles_180degrees::ViewAngles180Degrees::new()),
        // Box::new(viewangles_to_csv::ViewAnglesToCSV::new()),
        // Box::new(write_to_file::WriteToFile::new()),
    ]
}

pub fn get_experimental_algorithms() -> Vec<Box<dyn CheatAlgorithm + Send>> {
    Vec::new()
}
