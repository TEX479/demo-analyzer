// Written by Nocrex

use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::{Parameter, Parameters, get_parameter_value},
    },
    util::helpers::player_finders::{players_alive_and_in_pvs, valid_players},
};
use anyhow::Error;
use serde_json::json;
use std::collections::HashMap;
use steamid_ng::SteamID;
use tf_demo_parser::{ParserState, demo::message::Message};

pub struct OOBPitch {
    last_detections: Vec<SteamID>,
    server_name: String,

    params: Parameters,
}

impl OOBPitch {
    pub fn new() -> Self {
        let analyser: OOBPitch = OOBPitch {
            last_detections: Vec::new(),
            server_name: "".to_string(),
            params: HashMap::from([
                ("min_pitch".to_string(), Parameter::Float(-89.999)),
                ("max_pitch".to_string(), Parameter::Float(89.999)),
            ]),
        };
        analyser
    }
}

impl Default for OOBPitch {
    fn default() -> Self {
        Self::new()
    }
}

impl CheatAlgorithm for OOBPitch {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
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

            let min_pitch: f32 = get_parameter_value(&self.params, "min_pitch")?;
            let max_pitch: f32 = get_parameter_value(&self.params, "max_pitch")?;

            let is_valve_server = self.server_name.starts_with("Valve Matchmaking Server");

            for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(state.players.iter())) {
                if !(min_pitch..=max_pitch).contains(&player.pitch_angle) {
                    detections.push(steam_id);
                    if !self.last_detections.contains(&steam_id) {
                        submitted_detections.push(Detection {
                            tick: ticknum,
                            algorithm: self.algorithm_name().to_string(),
                            player: steam_id,
                            data: json!({
                                "pitch": player.pitch_angle,
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

    fn params(&mut self) -> Option<&mut Parameters> {
        Some(&mut self.params)
    }
}
