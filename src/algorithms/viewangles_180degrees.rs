use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::algorithm::{CheatAlgorithm, Detection},
    util::{
        helpers::{
            KeyValueVec, ViewAngle,
            player_finders::{players_alive_and_in_pvs, valid_players},
        },
        nocrex::jankguard::JankGuard,
    },
};
use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

// This example file looks for any examples of players rotating 180 degrees within a single server tick.

// To start, define a struct containing any information you want to store/share between events.
// Here we want to track the view angle and pitch angle of each player on the previous tick.
// Later we will compare the previous and current view angles to see if they are 180 degrees apart.
#[derive(Default)]
pub struct ViewAngles180Degrees {
    previous_angles: KeyValueVec<SteamID, ViewAngle>,
    previous_tick: u32,
}

// Then implement a pub fn new for your struct.
// Use the new() function to initalize any variables specified in the struct.
// IMPORTANT: new() gets called even if the algorithm is not selected! Don't do any non-ephemeral operations here; use CheatAlgorithm::init() instead.
// Additional helper functions and consts also go here.
impl ViewAngles180Degrees {
    pub fn new() -> Self {
        Self::default()
    }
}

// Implement the CheatAlgorithm trait. This is where the bulk of your algorithm resides.
// Any interesting detections should be documented in a Detection object and returned within a vector.
// You can attach whatever json data you want to each detection via the "data" field.
// You don't have to implement every function in CheatAlgorithm; see its definition for a complete list of functions.
impl CheatAlgorithm for ViewAngles180Degrees {
    // REQUIRED: Should this algorithm run by default if -a isn't specified?
    // Generally should be true, unless you're doing dev-only stuff (writing to files, printing debug output, etc).
    fn enabled_by_default(&self) -> bool {
        false
    }

    // REQUIRED: Set your algorithm's name here. Best practice is to match the filename.
    fn algorithm_name(&self) -> &'static str {
        "viewangles_180degrees"
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, _jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);
        let tick_delta = ticknum - self.previous_tick;
        let mut new_player_history = KeyValueVec::new();
        let mut detections = Vec::new();

        // In the vast majority of cases you will only want to iterate over players that are:
        // - In PVS (data is being sent to the client)
        // - Alive (you can't cheat if you're dead)
        // - Not a tf_bot (you can't convict a tf_bot)
        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(state.players.iter())) {
            let current_viewangle = player.view_angle;
            _ = new_player_history.insert(steam_id, current_viewangle);

            let Some(previous_viewangle) = self.previous_angles.get(&steam_id).copied() else {
                continue;
            };

            let va_delta = previous_viewangle.viewangle_delta(current_viewangle) / tick_delta.max(1) as f32;

            // Creating the detection object
            // Avoid creating multiple detection objects for the same player and tick.
            // Nothing will break if you do, but it will overrepresent the data point.
            if va_delta.yaw.abs() > 179.0 || va_delta.pitch.abs() >= 179.0 {
                detections.push(Detection {
                    tick: ticknum,
                    algorithm: self.algorithm_name(),
                    player: steam_id,
                    data: json!({ "delta_yaw": va_delta.yaw, "delta_pitch": va_delta.pitch }),
                });
            }
        }
        self.previous_tick = state.tick.into();
        self.previous_angles = new_player_history;
        // Any detections returned are official and final!
        // If you don't want to return any detections, just return an empty vector.
        // If your algorithm needs future ticks, you can store the detections within your algorithm's struct.
        // You can then return them in a later CheatAlgorithm::on_tick() or in CheatAlgorithm::finish().
        Ok(detections)
    }
}
