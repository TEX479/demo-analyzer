// Written by Nocrex, Patched for Command Batching by Ciam

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
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct AngleRepeat {
    // TODO: replace with custom history struct
    ticks: Vec<(u32, KeyValueVec<SteamID, ViewAngle>)>,

    params: Parameters,
    detections: Vec<Detection>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub min_angle_diff_ratio: f32,
    pub min_first_second_angle_delta: f32,
    pub max_first_third_angle_delta: f32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            min_angle_diff_ratio: 3.0,
            min_first_second_angle_delta: 0.0,
            max_first_third_angle_delta: 0.028,
        }
    }
}

impl AngleRepeat {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheatAlgorithm for AngleRepeat {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        "nocrex/angle_repeat"
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        self.ticks.insert(0, (ticknum, KeyValueVec::default()));
        self.ticks.truncate(3);

        let min_angle_diff_ratio = self.params.min_angle_diff_ratio;
        let min_first_second_angle_delta = self.params.min_first_second_angle_delta;
        let max_first_third_angle_delta = self.params.max_first_third_angle_delta;

        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(players.iter())) {
            let prev_data = self.ticks.get(1).and_then(|(t, m)| m.get(&steam_id).map(|a| (*t, *a)));
            let second_prev_data = self.ticks.get(2).and_then(|(t, m)| m.get(&steam_id).map(|a| (*t, *a)));

            let ticks_since_event = jg.teleported(&steam_id, ticknum).min(jg.spawned(&steam_id, ticknum));

            if ticks_since_event < EVENT_TIMEOUT {
                // Ignore detections +-60 ticks from a teleport or spawn event
                if ticks_since_event == 0 {
                    self.detections.retain(|det| det.player != steam_id || (ticknum - det.tick) > EVENT_TIMEOUT);
                }
                continue;
            }

            let third_angle = player.view_angle;
            _ = self
                .ticks
                .get_mut(0)
                .expect("the first tick was inserted at the start of this function")
                .1
                .insert(steam_id, third_angle); // Store angle of this tick for next ticks

            if let (Some((second_t, second_angle)), Some((first_t, first_angle))) = (prev_data, second_prev_data) {
                let first_second_delta = calc_real_delta(first_t, first_angle, second_t, second_angle);
                let first_third_delta = calc_real_delta(first_t, first_angle, ticknum, third_angle);

                if first_second_delta < min_first_second_angle_delta {
                    // Ignore players with only a tiny adjustment in second angle
                    continue;
                }

                let ratio = first_second_delta / first_third_delta.max(1.0);

                if first_third_delta <= max_first_third_angle_delta && ratio > min_angle_diff_ratio && jg.fired(&steam_id, ticknum) < 3 {
                    self.detections.push(Detection {
                        tick: ticknum,
                        algorithm: self.algorithm_name(),
                        player: steam_id,
                        data: json!({
                            "angle_1": first_angle,
                            "angle_2": second_angle,
                            "angle_3": third_angle,
                            "1_3_delta": first_third_delta,
                            "1_2_delta": first_second_delta,
                            "ratio": ratio,
                        }),
                    });
                }
            }
        }

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
        let params = make_algorithm_params!(self.params; [min_angle_diff_ratio, min_first_second_angle_delta, max_first_third_angle_delta]);
        Some(params)
    }
}

fn calc_real_delta(tick_old: u32, angle_old: ViewAngle, tick_new: u32, angle_new: ViewAngle) -> f32 {
    let tick_delta = tick_new.saturating_sub(tick_old).max(1);

    let va_real = angle_old.viewangle_delta(angle_new) / tick_delta as f32;

    va_real.magnitude()
}
