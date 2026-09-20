// Written by Teltta
use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::{Parameter, Parameters, get_parameter_value},
    },
};
use anyhow::Error;
use serde_json::{Map, Value};
use std::collections::HashMap;
use steamid_ng::SteamID;
use tf_demo_parser::{
    ParserState,
    demo::{gameevent_gen::GameEvent, message::Message, parser::analyser::UserId},
};

#[derive(Default)]
pub struct DoubleTap {
    params: Parameters,

    shots: HashMap<SteamID, Shot>,
}

impl DoubleTap {
    pub fn new() -> Self {
        Self {
            params: HashMap::from([
                ("assert_pvs".to_string(), Parameter::Bool(true)),
                ("min_tick_scout".to_string(), Parameter::Int(17)),
                ("min_tick_heavy".to_string(), Parameter::Int(3)),
            ]),
            shots: HashMap::new(),
        }
    }

    fn get_weapon_mapping(&self, weapon_id: u32) -> Option<u32> {
        match weapon_id {
            16 => get_parameter_value::<i32>(&self.params, "min_tick_scout").ok(),
            18 => get_parameter_value::<i32>(&self.params, "min_tick_heavy").ok(),
            _ => None,
        }
        .map(|x| x as u32)
    }
}

fn is_cleaver_or_wrap_assassin(weapon_id: u32, damage: u32) -> bool {
    weapon_id == 16 && (damage == 50 || (3..=8).contains(&damage))
}

impl CheatAlgorithm for DoubleTap {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "teltta/doubletap"
    }

    fn on_tick(&mut self, _: &CheatAnalyserState, _: &ParserState) -> Result<Vec<Detection>, Error> {
        Ok(vec![])
    }

    fn handled_messages(&self) -> Result<Vec<tf_demo_parser::MessageType>, bool> {
        Ok(vec![tf_demo_parser::MessageType::GameEvent])
    }

    fn on_message(
        &mut self,
        message: &tf_demo_parser::demo::message::Message,
        state: &CheatAnalyserState,
        _: &ParserState,
        tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        let mut detections = Vec::new();
        let ticknum = u32::from(tick);
        let players = &state.players;

        if let Message::GameEvent(event_msg) = message
            && let GameEvent::PlayerHurt(hurt) = &event_msg.event
        {
            let dmg = hurt.damage_amount as u32;
            let weapon = hurt.weapon_id as u32;

            // ignore wrap assassin bleed
            if is_cleaver_or_wrap_assassin(weapon, dmg) {
                return Ok(vec![]);
            }

            let attacker_uid = UserId::from(hurt.attacker);
            let victim_uid = UserId::from(hurt.user_id);

            // get steam id64 from uids
            let Some(attacker_sid) = state.get_id64_from_userid(attacker_uid) else {
                return Ok(Vec::new());
            };
            let Some(victim_sid) = state.get_id64_from_userid(victim_uid) else {
                return Ok(Vec::new());
            };

            let Some(attacker) = players.iter().find(|x| {
                x.info
                    .as_ref()
                    .is_some_and(|info| SteamID::from_steam3(&info.steam_id).unwrap_or_default() == attacker_sid)
            }) else {
                return Ok(vec![]);
            };

            if !attacker.in_pvs {
                let assert_pvs = get_parameter_value::<bool>(&self.params, "assert_pvs")?;
                if assert_pvs {
                    return Ok(vec![]);
                }
            }

            let Some(past_shot) = self.shots.insert(
                attacker_sid,
                Shot {
                    tick: ticknum,
                    weapon,
                    victim_uid,
                    dmg,
                },
            ) else {
                return Ok(vec![]);
            };

            let Shot {
                tick: past_tick,
                weapon: past_weapon,
                victim_uid: past_victim,
                dmg: past_dmg,
            } = past_shot;
            let Some(min_diff) = self.get_weapon_mapping(weapon) else {
                return Ok(vec![]);
            };

            if past_weapon != weapon || past_victim != victim_uid {
                return Ok(vec![]);
            }

            let diff = ticknum - past_tick;

            if diff < min_diff && diff > 0 {
                let u200b = "\u{200b}";
                let data: Vec<(&str, Value)> = vec![
                    ("class", Value::from(attacker.class.to_string())),
                    ("tick_1", Value::from(past_tick)),
                    ("tick_2", Value::from(ticknum)),
                    ("tick_diff", Value::from(diff)),
                    ("victim", Value::from(victim_sid.steam3())),
                    ("weapon_id", Value::from(weapon)),
                    ("damage_1", Value::from(past_dmg)),
                    ("damage_2", Value::from(dmg)),
                ];
                let mut new_data = Map::new();
                for (i, (key, value)) in data.into_iter().enumerate() {
                    new_data.insert(format!("{}{}", u200b.repeat(i), key), value);
                }
                let new_data = Value::Object(new_data);

                detections.push(Detection {
                    tick: tick.into(),
                    algorithm: self.algorithm_name().to_string(),
                    player: attacker_sid,
                    data: new_data,
                });
            }
        }
        Ok(detections)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(vec![])
    }

    fn params(&mut self) -> Option<&mut Parameters> {
        Some(&mut self.params)
    }
}

#[derive(Debug, Clone, Copy)]
struct Shot {
    pub tick: u32,
    pub weapon: u32,
    pub victim_uid: UserId,
    pub dmg: u32,
}
