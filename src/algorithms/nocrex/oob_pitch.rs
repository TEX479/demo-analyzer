// Written by Nocrex

use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::AlgorithmParameters,
    },
    make_algorithm_params,
    util::{
        helpers::player_finders::{players_alive_and_in_pvs, valid_players},
        nocrex::jankguard::JankGuard,
    },
};
use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::{ParserState, demo::message::Message};

#[derive(Debug, Clone, Default)]
pub struct OOBPitch {
    last_detections: Vec<SteamID>,
    server_name: String,

    params: Parameters,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub min_pitch: f32,
    pub max_pitch: f32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            min_pitch: -89.999,
            max_pitch: 89.999,
        }
    }
}

impl OOBPitch {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheatAlgorithm for OOBPitch {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        "nocrex/oob_pitch"
    }

    fn handled_messages(&self) -> Result<Vec<tf_demo_parser::MessageType>, bool> {
        Ok(vec![tf_demo_parser::MessageType::ServerInfo, tf_demo_parser::MessageType::NetTick])
    }

    fn on_message(
        &mut self,
        message: &Message,
        state: &CheatAnalyserState,
        _: &ParserState,
        _: &JankGuard,
        _: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        let mut submitted_detections = Vec::new();

        if let Message::ServerInfo(event) = message
            && self.server_name.is_empty()
        {
            self.server_name = event.server_name.trim().to_string();
        }

        if let Message::NetTick(_) = message {
            let ticknum = u32::from(state.tick);

            let mut detections = Vec::new();

            let min_pitch = self.params.min_pitch;
            let max_pitch = self.params.max_pitch;

            let is_valve_server = self.server_name.starts_with("Valve Matchmaking Server");

            for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(state.players.iter())) {
                if !(min_pitch..=max_pitch).contains(&player.view_angle.pitch) {
                    detections.push(steam_id);
                    if !self.last_detections.contains(&steam_id) {
                        submitted_detections.push(Detection {
                            tick: ticknum,
                            algorithm: self.algorithm_name(),
                            player: steam_id,
                            data: json!({
                                "pitch": player.view_angle.pitch,
                                "valve_server": is_valve_server
                            }),
                        });
                    }
                }
            }

            self.last_detections = detections;
        }

        Ok(submitted_detections)
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [min_pitch, max_pitch]);
        Some(params)
    }
}
