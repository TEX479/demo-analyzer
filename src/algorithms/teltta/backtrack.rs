// Written by Teltta
use crate::base::cheat_analyser_base::CheatAnalyserState;
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::helpers::player_finders::player_with_sid;
use crate::util::nocrex::jankguard::JankGuard;
use anyhow::Error;
use serde_json::{Map, Value};
use tf_demo_parser::ParserState;
use tf_demo_parser::demo::gameevent_gen::GameEvent;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::parser::analyser::UserId;

#[derive(Default)]
pub struct BackTrack {
    params: Parameters,
}

impl BackTrack {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub distance: f32,
    pub max_distance: f32,
    pub max_angle_diff: f32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            distance: 200.0,
            max_distance: 600.0,
            max_angle_diff: 110.0,
        }
    }
}

fn angle_diff(a: f32, b: f32) -> f32 {
    (b - a + 180.0).rem_euclid(360.0) - 180.0
}

fn is_backstab(damage: u16, is_crit: bool, weapon_id: u16, victim_health: u16) -> bool {
    is_crit && weapon_id == 7 && (damage > 600 || damage as f32 > victim_health as f32 * 5.5)
}

impl CheatAlgorithm for BackTrack {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "teltta/backtrack"
    }

    fn on_tick(&mut self, _: &CheatAnalyserState, _: &ParserState, _jg: &JankGuard) -> Result<Vec<Detection>, Error> {
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
        _: &JankGuard,
        tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        let mut detections = Vec::new();

        if let Message::GameEvent(event_msg) = message
            && let GameEvent::PlayerHurt(hurt) = &event_msg.event
        {
            let victim_uid = UserId::from(hurt.user_id);
            let attacker_uid = UserId::from(hurt.attacker);

            let is_crit = hurt.crit;

            let weapon_id = hurt.weapon_id;
            let damage_amount = hurt.damage_amount;

            let distance = self.params.distance;
            let max_distance = self.params.max_distance;
            let max_angle_diff = self.params.max_angle_diff;

            // get steam id64 from uids
            let Some(attacker_sid) = state.get_steamid_from_userid(attacker_uid) else {
                return Ok(Vec::new());
            };
            let Some(victim_sid) = state.get_steamid_from_userid(victim_uid) else {
                return Ok(Vec::new());
            };

            let players = &state.players;

            let Some((attacker, _attacker_userinfo)) = player_with_sid(players.iter(), attacker_sid) else {
                return Ok(vec![]);
            };
            let attacker_pos = attacker.position;

            // PERF: on paper, reversing the second search improves worst- (and best-) case speeds.
            //       if this does not hold up or the compiler can dom some weird stuff, remove.
            let Some((victim, _victim_userinfo)) = player_with_sid(players.iter().rev(), victim_sid) else {
                return Ok(vec![]);
            };
            let victim_pos = victim.position;
            let victim_health = victim.health;

            if attacker_uid != victim_uid && is_backstab(damage_amount, is_crit, weapon_id, victim_health) {
                let angle_diff = angle_diff(attacker.view_angle.yaw, victim.view_angle.yaw).abs();

                // 3d distance
                // let pos_diff = (f32::powi(attacker_pos.x - victim_pos.x, 2) +
                //                     f32::powi(attacker_pos.y - victim_pos.y, 2) +
                //                     f32::powi(attacker_pos.z - victim_pos.z, 2)).sqrt();

                // 2d distance
                let pos_diff = (f32::powi(attacker_pos.x - victim_pos.x, 2) + f32::powi(attacker_pos.y - victim_pos.y, 2)).sqrt();

                if pos_diff < max_distance && (angle_diff > max_angle_diff || pos_diff > distance) {
                    // let data = json!({
                    //     "angle_attacker": attacker.view_angle,
                    //     "angle_victim": victim.view_angle,
                    //     "angle_diff": angle_diff,
                    //     "pos_attacker": attacker_pos,
                    //     "pos_victim": victim_pos,
                    //     "distance": pos_diff,
                    //     "damage": damage_amount,
                    //     "type": if angle_diff > max_angle_diff && pos_diff < max_distance {"Angle"} else {"Distance"},
                    //     "victim_class": victim.class,
                    //     "victim_health": victim_health,
                    // });

                    // ########################

                    // TODO: understand (or change) the following part. (like *why* is it the way it is)
                    let u200b = "\u{200b}";
                    let data: Vec<(&str, Value)> = vec![
                        ("angle_attacker", Value::from(attacker.view_angle.yaw)),
                        ("angle_victim", Value::from(victim.view_angle.yaw)),
                        ("angle_diff", Value::from(angle_diff)),
                        ("pos_attacker", Value::from(vec![attacker_pos.x, attacker_pos.y, attacker_pos.z])),
                        ("pos_victim", Value::from(vec![victim_pos.x, victim_pos.y, victim_pos.z])),
                        ("distance", Value::from(pos_diff)),
                        ("damage", Value::from(damage_amount)),
                        (
                            "type",
                            Value::from(if angle_diff > max_angle_diff && pos_diff < max_distance {
                                "Angle"
                            } else {
                                "Distance"
                            }),
                        ),
                        ("victim_class", Value::from(victim.class.to_string())),
                        ("victim_health", Value::from(victim.health)),
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
        }
        Ok(detections)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(vec![])
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [distance, max_distance, max_angle_diff]);
        Some(params)
    }
}
