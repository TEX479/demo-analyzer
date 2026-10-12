// TODO: refactor more
use std::collections::HashMap;

use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

use crate::base::cheat_analyser_base::CheatAnalyserState;
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::helpers::Vec3;
use crate::util::helpers::player_finders::{players_alive_and_in_pvs, valid_players};
use crate::util::nocrex::jankguard::JankGuard;

#[derive(Debug, Default, Clone)]
struct PlayerBhopState {
    ground_ticks: u32,
    air_ticks: u32,
    air_ticks_at_landing: u32,
    current_streak: u32,
    hop_ground_ticks: Vec<u32>,
    hop_speeds: Vec<f32>,
    last_reported_streak: u32,
    prev_pos: Option<Vec3>,
    prev_on_ground: bool,
    last_movement_dz: f32,
    launch_z: f32,
    apex_z: f32,
    last_pos_sample: Option<Vec3>,
    last_pos_sample_tick: u32,
    current_speed_hu_s: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub min_streak: usize,
    pub min_air_ticks: usize,
    pub max_ground_ticks_bhop: usize,
    pub min_speed: f32,
}

#[derive(Debug, Clone, Default)]
pub struct BunnyHop {
    player_states: HashMap<SteamID, PlayerBhopState>,
    params: Parameters,
    detections: Vec<Detection>,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            min_streak: 5,
            min_air_ticks: 6,
            max_ground_ticks_bhop: 1,
            min_speed: 150.0,
        }
    }
}

impl BunnyHop {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheatAlgorithm for BunnyHop {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "fidoo/bunnyhop"
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [min_streak, min_air_ticks, max_ground_ticks_bhop, min_speed]);
        Some(params)
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        let min_streak = self.params.min_streak;
        let min_air_ticks = self.params.min_air_ticks;
        let max_ground_ticks = self.params.max_ground_ticks_bhop;
        let min_speed = self.params.min_speed;

        let mut tick_detections = Vec::new();
        let algo_name = self.algorithm_name().to_string();

        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(players.iter())) {
            let ticks_since_event = jg.teleported(&steam_id, ticknum).min(jg.spawned(&steam_id, ticknum));

            // Ignore detections within 60 ticks of spawn or teleport
            if ticks_since_event < 60 {
                self.player_states.remove(&steam_id);
                continue;
            }

            // Swimming in water alters ground/air flag behavior
            if player.is_in_water() {
                self.player_states.remove(&steam_id);
                continue;
            }

            let pstate = self.player_states.entry(steam_id).or_default();

            let on_ground = player.is_on_ground();
            let prev_z = pstate.prev_pos.map_or(player.position.z, |p| p.z);
            let dz = player.position.z - prev_z;

            let (dx, dy) = if let Some(prev_pos) = pstate.prev_pos {
                (player.position.x - prev_pos.x, player.position.y - prev_pos.y)
            } else {
                (0.0, 0.0)
            };
            let pos_delta_sq = dx * dx + dy * dy;

            // When a position packet arrives, compute accurate speed across update interval
            let pos_updated = dz.abs() > 0.001 || pos_delta_sq > 0.001;

            if pos_updated {
                let tick_delta = if pstate.last_pos_sample_tick > 0 {
                    (ticknum - pstate.last_pos_sample_tick).max(1)
                } else {
                    1
                };

                if let Some(last_sample) = pstate.last_pos_sample {
                    let sample_dx = player.position.x - last_sample.x;
                    let sample_dy = player.position.y - last_sample.y;
                    let dist = (sample_dx * sample_dx + sample_dy * sample_dy).sqrt();
                    pstate.current_speed_hu_s = (dist / (tick_delta as f32)) * 66.66667;
                }

                pstate.last_pos_sample = Some(player.position);
                pstate.last_pos_sample_tick = ticknum;
            }

            let current_speed = pstate.current_speed_hu_s;

            if on_ground {
                pstate.launch_z = player.position.z;
                pstate.apex_z = player.position.z;

                if !pstate.prev_on_ground {
                    pstate.air_ticks_at_landing = pstate.air_ticks;
                    pstate.ground_ticks = 1;
                    pstate.air_ticks = 0;
                } else {
                    pstate.ground_ticks += 1;
                    if pstate.ground_ticks > (max_ground_ticks.max(1) + 3) as u32 {
                        // Walking on ground breaks the bunnyhop streak
                        pstate.current_streak = 0;
                        pstate.hop_ground_ticks.clear();
                        pstate.hop_speeds.clear();
                        pstate.last_reported_streak = 0;
                    }
                }
            } else {
                pstate.apex_z = pstate.apex_z.max(player.position.z);

                // Check for zero-tick ground bounce (instant jump impulse without FL_ONGROUND toggle)
                // Filter out Scout mid-air double/triple jumps:
                // Must have fallen >= 20 units from apex down to ground level (prev_z) before jumping up
                let fallen_from_apex = pstate.apex_z - prev_z;
                let near_ground_level = prev_z <= pstate.launch_z + 25.0;

                let is_ground_bounce = pos_updated
                    && !pstate.prev_on_ground
                    && pstate.last_movement_dz < -1.0
                    && dz > 5.0
                    && pstate.air_ticks >= min_air_ticks as u32
                    && fallen_from_apex >= 20.0
                    && near_ground_level;

                if is_ground_bounce {
                    // TODO: check if `0 <= max_ground_ticks` was meant to do something
                    if current_speed >= min_speed {
                        pstate.current_streak += 1;
                        pstate.hop_ground_ticks.push(0);
                        pstate.hop_speeds.push(current_speed);

                        pstate.air_ticks_at_landing = pstate.air_ticks;
                        pstate.air_ticks = 1;
                        pstate.ground_ticks = 0;
                        pstate.launch_z = player.position.z;
                        pstate.apex_z = player.position.z;

                        if pstate.current_streak >= min_streak as u32
                            && pstate.current_streak > pstate.last_reported_streak
                            && pstate.hop_ground_ticks.iter().all(|&g| g <= max_ground_ticks as u32)
                        {
                            pstate.last_reported_streak = pstate.current_streak;
                            let avg_speed = pstate.hop_speeds.iter().sum::<f32>() / pstate.hop_speeds.len() as f32;
                            let max_speed = pstate.hop_speeds.iter().cloned().fold(0.0_f32, f32::max);
                            let detection = Detection {
                                tick: ticknum,
                                algorithm: algo_name.clone(),
                                player: steam_id,
                                data: json!({
                                    "class": player.class_name(),
                                    "weapon": state.get_player_weapon(player),
                                    "streak": pstate.current_streak,
                                    "ground_ticks": pstate.hop_ground_ticks.clone(),
                                    "avg_speed": avg_speed,
                                    "max_speed": max_speed,
                                    "speed": current_speed,
                                }),
                            };
                            tick_detections.push(detection.clone());
                            self.detections.push(detection);
                        }
                    } else {
                        pstate.current_streak = 0;
                        pstate.hop_ground_ticks.clear();
                        pstate.hop_speeds.clear();
                        pstate.last_reported_streak = 0;
                    }
                } else if pstate.prev_on_ground {
                    let hopped_ground_ticks = pstate.ground_ticks;
                    let air_at_landing = pstate.air_ticks_at_landing;
                    pstate.ground_ticks = 0;
                    pstate.air_ticks = 1;
                    pstate.launch_z = player.position.z;
                    pstate.apex_z = player.position.z;

                    if air_at_landing >= min_air_ticks as u32 && current_speed >= min_speed {
                        // Strictly enforce that this hop does not exceed max_ground_ticks
                        if hopped_ground_ticks as usize <= max_ground_ticks {
                            pstate.current_streak += 1;
                            pstate.hop_ground_ticks.push(hopped_ground_ticks);
                            pstate.hop_speeds.push(current_speed);

                            if pstate.current_streak >= min_streak as u32
                                && pstate.current_streak > pstate.last_reported_streak
                                && pstate.hop_ground_ticks.iter().all(|&g| g <= max_ground_ticks as u32)
                            {
                                pstate.last_reported_streak = pstate.current_streak;
                                let avg_speed = pstate.hop_speeds.iter().sum::<f32>() / pstate.hop_speeds.len() as f32;
                                let max_speed = pstate.hop_speeds.iter().cloned().fold(0.0_f32, f32::max);
                                let detection = Detection {
                                    tick: ticknum,
                                    algorithm: algo_name.clone(),
                                    player: steam_id,
                                    data: json!({
                                        "class": player.class_name(),
                                        "weapon": state.get_player_weapon(player),
                                        "streak": pstate.current_streak,
                                        "ground_ticks": pstate.hop_ground_ticks.clone(),
                                        "avg_speed": avg_speed,
                                        "max_speed": max_speed,
                                        "speed": current_speed,
                                    }),
                                };
                                tick_detections.push(detection.clone());
                                self.detections.push(detection);
                            }
                        } else {
                            // Ground contact exceeded max_ground_ticks -> break streak
                            pstate.current_streak = 0;
                            pstate.hop_ground_ticks.clear();
                            pstate.hop_speeds.clear();
                            pstate.last_reported_streak = 0;
                        }
                    }
                } else {
                    pstate.air_ticks += 1;
                }
            }

            if pos_updated {
                pstate.last_movement_dz = dz;
            }
            pstate.prev_pos = Some(player.position);
            pstate.prev_on_ground = on_ground;
        }

        Ok(tick_detections)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(vec![])
    }
}
