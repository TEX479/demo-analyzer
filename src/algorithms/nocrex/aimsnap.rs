// Written by Nocrex, Patched for Command Batching by Ciam, Refactored by tex
//
// Checks for single tick snaps, which may occur on:
// - aimlocking onto a target
// - snapping up/down for auto rocketjump
// - maybe some other cheats

// PERF: check why this algorithm eats performance and if it can be improved, fix that

use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::AlgorithmParameters,
    },
    make_algorithm_params,
    util::{
        helpers::{
            KeyValueVec, ViewAngle,
            player_finders::{players_alive_and_in_pvs, valid_players},
        },
        nocrex::jankguard::{EVENT_TIMEOUT, JankGuard},
    },
};
use anyhow::Error;
use serde_json::json;
use std::ops::Range;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct AimSnap {
    /// history of the last 5 ticks
    players: KeyValueVec<SteamID, AngleHistory>,
    params: Parameters,
    detections: Vec<Detection>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub noise_min: f32,
    pub noise_max: f32,
    pub snap_threshold: f32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            // TODO: check why noise_min is not 0
            noise_min: 0.028,
            noise_max: 0.99,
            snap_threshold: 10.0,
        }
    }
}

impl AimSnap {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheatAlgorithm for AimSnap {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        "nocrex/aimsnap"
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        let noise_min = self.params.noise_min;
        let noise_max = self.params.noise_max;
        let snap_threshold = self.params.snap_threshold;

        let noise_range: Range<f32> = noise_min..noise_max;

        let mut players_to_remove = self.players.keys().to_vec();

        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(players.iter())) {
            let ticks_since_event = jg.teleported(&steam_id, ticknum).min(jg.spawned(&steam_id, ticknum));
            if ticks_since_event < EVENT_TIMEOUT {
                // Ignore detections +-EVENT_TIMEOUT ticks from a teleport or spawn event
                if ticks_since_event == 0 {
                    self.detections.retain(|det| det.player != steam_id || (ticknum - det.tick) > EVENT_TIMEOUT);
                }
                continue;
            }

            if let Some(i) = players_to_remove.iter().enumerate().find(|(_, sid)| **sid == steam_id).map(|(i, _)| i) {
                players_to_remove.swap_remove(i);
            }

            let angle_history = self.players.get_mut_or_insert_default(steam_id);
            let Some(delta_magnitudes) = angle_history.push(player.view_angle) else {
                continue;
            };

            if noise_range.contains(&delta_magnitudes[0])
                && delta_magnitudes[1] > snap_threshold
                && noise_range.contains(&delta_magnitudes[2])
                && jg.fired(&steam_id, ticknum) < 5
            {
                self.detections.push(Detection {
                    tick: ticknum - 2,
                    algorithm: self.algorithm_name(),
                    player: steam_id,
                    data: json!({
                        "deltas": delta_magnitudes
                    }),
                });
            }
        }

        players_to_remove.iter().for_each(|sid| _ = self.players.remove(sid));

        let detections_final = self
            .detections
            .iter()
            .filter(|d| ticknum.saturating_sub(d.tick) >= EVENT_TIMEOUT)
            .cloned()
            .collect();
        self.detections.retain(|d| ticknum.saturating_sub(d.tick) < EVENT_TIMEOUT);

        Ok(detections_final)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(std::mem::take(&mut self.detections))
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [noise_min, noise_max, snap_threshold]);
        Some(params)
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum AngleHistory {
    #[default]
    None,
    One([ViewAngle; 1]),
    Two([ViewAngle; 2]),
    Three([ViewAngle; 3]),
    Four([ViewAngle; 4]),
}

impl AngleHistory {
    pub fn push(&mut self, view_angle: ViewAngle) -> Option<[f32; 3]> {
        let (new_self, delta_magnitudes) = match *self {
            Self::None => (Self::One([view_angle]), None),
            Self::One([v1]) => (Self::Two([v1, view_angle]), None),
            Self::Two([v1, v2]) => (Self::Three([v1, v2, view_angle]), None),
            Self::Three([v1, v2, v3]) => {
                let viewangles = [v1, v2, v3, view_angle];
                (Self::Four(viewangles), Some(viewangles_to_magnitudes(viewangles)))
            }
            Self::Four([_, v1, v2, v3]) => {
                let viewangles = [v1, v2, v3, view_angle];
                (Self::Four(viewangles), Some(viewangles_to_magnitudes(viewangles)))
            }
        };
        *self = new_self;
        delta_magnitudes
    }
}

fn viewangles_to_magnitudes(viewangles: [ViewAngle; 4]) -> [f32; 3] {
    [
        viewangles[0].viewangle_delta(viewangles[1]).magnitude(),
        viewangles[1].viewangle_delta(viewangles[2]).magnitude(),
        viewangles[2].viewangle_delta(viewangles[3]).magnitude(),
    ]
}
