use crate::{
    base::cheat_analyser_base::{Player, PlayerState},
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::{Parameter, Parameters},
    },
    util::nocrex::jankguard::JankGuard,
};
use anyhow::Context;
use serde::Serialize;
use std::{
    collections::HashMap,
    ops::{Add, Neg, Sub},
};
use steamid_ng::SteamID;

static PARAMETER_THRESHOLD: &str = "threshold";
static PARAMETER_MAX_FIRE_DELAY: &str = "max_fire_delay";

#[derive(Debug, Clone)]
pub struct AngleAccelerationAnalyzer {
    angle_history: HashMap<SteamID, PreviousAngles>,
    // ticks: VecDeque<HashMap<u64, Player>>,
    parameters: Parameters,

    jg: JankGuard,
    detections: Vec<Detection>,
}

impl Default for AngleAccelerationAnalyzer {
    fn default() -> Self {
        Self {
            angle_history: Default::default(),
            parameters: Parameters::from([
                // TODO: fine tune the default params
                (PARAMETER_THRESHOLD.to_string(), Parameter::Float(15.0)),
                (PARAMETER_MAX_FIRE_DELAY.into(), Parameter::Int(1)),
            ]),
            jg: Default::default(),
            detections: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum PreviousAngles {
    #[default]
    None,
    OneAngle(Angle),
    AngleAndVelocity {
        previous_angle: Angle,
        previous_velocity: AngleVelocity,
    },
}

impl PreviousAngles {
    pub fn insert_angle(&mut self, angle: Angle) -> Option<AngleAcceleration> {
        let (new_self, ret_val) = match self {
            Self::None => (Self::OneAngle(angle), None),
            Self::OneAngle(previous_angle) => (
                Self::AngleAndVelocity {
                    previous_angle: angle,
                    previous_velocity: AngleVelocity::from_angles(*previous_angle, angle),
                },
                None,
            ),
            Self::AngleAndVelocity {
                previous_angle,
                previous_velocity,
            } => {
                let new_velocity = AngleVelocity::from_angles(*previous_angle, angle);
                let new_self = Self::AngleAndVelocity {
                    previous_angle: angle,
                    previous_velocity: new_velocity,
                };
                (new_self, Some(AngleAcceleration::from_angle_velocities(*previous_velocity, new_velocity)))
            }
        };
        *self = new_self;
        ret_val
    }

    pub fn reset(&mut self) {
        *self = Self::None;
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
struct Angle {
    /// left/right
    yaw: f32,
    /// up/down
    pitch: f32,
}

#[derive(Debug, Clone, Copy)]
struct AngleVelocity(Angle);

#[derive(Debug, Clone, Copy)]
struct AngleAcceleration(Angle);

impl From<&Player> for Angle {
    fn from(value: &Player) -> Self {
        let yaw = value.view_angle % 360.0;
        let pitch = value.pitch_angle % 360.0;
        Self { yaw, pitch }.normalized()
    }
}

impl From<Angle> for serde_json::Value {
    fn from(value: Angle) -> Self {
        let mut new_map = serde_json::Map::new();
        new_map.insert("yaw".to_string(), value.yaw.into());
        new_map.insert("pitch".to_string(), value.pitch.into());
        serde_json::Value::Object(new_map)
    }
}

impl Neg for Angle {
    type Output = Self;
    fn neg(self) -> Self::Output {
        let Self { yaw, pitch } = self;
        let yaw = yaw - 180.0;
        let pitch = pitch - 180.0;
        Self { yaw, pitch }
    }
}
impl Add for Angle {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            yaw: self.yaw + rhs.yaw,
            pitch: self.pitch + rhs.pitch,
        }
        .normalized()
    }
}
impl Sub for Angle {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            yaw: self.yaw - rhs.yaw,
            pitch: self.pitch - rhs.pitch,
        }
        .normalized()
    }
}

impl Angle {
    #[must_use]
    pub fn rotate_by(self, angle: Angle) -> Self {
        self - angle
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let Self { mut yaw, mut pitch } = self;
        yaw %= 360.0;
        pitch %= 360.0;

        // normalize angles between +-180°
        if yaw > 180.0 {
            yaw -= 360.0;
        } else if yaw < -180.0 {
            yaw += 360.0;
        }
        if pitch > 180.0 {
            pitch -= 360.0;
        } else if pitch < -180.0 {
            pitch += 360.0;
        }

        // rotate if
        if pitch > 90.0 {
            pitch = 90.0 - pitch;
            yaw += 180.0;
        } else if pitch < -90.0 {
            pitch = -90.0 - pitch;
            yaw += 180.0;
        }

        // in case yaw was modified renormalize
        yaw %= 360.0;
        if yaw > 180.0 {
            yaw -= 360.0;
        } else if yaw < -180.0 {
            yaw += 360.0;
        }

        Self { yaw, pitch }
    }

    /// magnitude of the deviation from (0, 0)
    ///
    /// for getting the strength of the acceleration
    pub fn magnitude(&self) -> f32 {
        (self.yaw.powi(2) + self.pitch.powi(2)).sqrt()
    }
}

impl AngleVelocity {
    pub fn from_angles(previous_tick: Angle, current_tick: Angle) -> Self {
        let angle = current_tick.rotate_by(previous_tick);
        Self(angle)
    }
}

impl AngleAcceleration {
    pub fn from_angle_velocities(previous_tick: AngleVelocity, current_tick: AngleVelocity) -> Self {
        let angle = current_tick.0.rotate_by(previous_tick.0);
        Self(angle)
    }
}

impl From<AngleAcceleration> for serde_json::Value {
    fn from(value: AngleAcceleration) -> Self {
        let mut new_map = serde_json::Map::new();
        new_map.insert("yaw".to_string(), value.0.yaw.into());
        new_map.insert("pitch".to_string(), value.0.pitch.into());
        new_map.insert("magnitude".to_string(), value.0.magnitude().into());
        serde_json::Value::Object(new_map)
    }
}

impl AngleAccelerationAnalyzer {
    pub fn get_parameter_int(&self, parameter_name: &str) -> anyhow::Result<i32> {
        match self
            .parameters
            .get(parameter_name)
            .context(format!("could not get parameter {:?}", parameter_name))?
        {
            Parameter::Int(x) => Ok(*x),
            parameter => Err(anyhow::Error::msg(format!(
                "expected parameter {:?} to be of type 'Int', got: {:?}",
                parameter_name, parameter
            ))),
        }
    }

    pub fn get_parameter_float(&self, parameter_name: &str) -> anyhow::Result<f32> {
        match self
            .parameters
            .get(parameter_name)
            .context(format!("could not get parameter {:?}", parameter_name))?
        {
            Parameter::Float(x) => Ok(*x),
            parameter => Err(anyhow::Error::msg(format!(
                "expected parameter {:?} to be of type 'Float', got: {:?}",
                parameter_name, parameter
            ))),
        }
    }
}

impl CheatAlgorithm for AngleAccelerationAnalyzer {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "angle_acceleration"
    }

    fn handled_messages(&self) -> Result<Vec<tf_demo_parser::MessageType>, bool> {
        self.jg.handled_messages()
    }

    fn on_tick(
        &mut self,
        state: &crate::base::cheat_analyser_base::CheatAnalyserState,
        _: &tf_demo_parser::ParserState,
    ) -> Result<Vec<Detection>, anyhow::Error> {
        self.jg.on_tick(state).context(format!("jankguard errored in on_tick with state {:?}", state))?;
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        let threshold = self
            .get_parameter_float(PARAMETER_THRESHOLD)
            .context("threshold must exist and be of the correct type")?;
        let max_fire_delay = self
            .get_parameter_int(PARAMETER_MAX_FIRE_DELAY)
            .context("max_fire_delay must exist and be of the correct type")? as u32;

        let mut handled_steam_ids: Vec<SteamID> = Vec::new();
        let mut detections = Vec::new();

        for player in players.iter().filter(|p| p.in_pvs && p.state == PlayerState::Alive) {
            let Some(info) = player.info.as_ref() else {
                continue;
            };
            if info.steam_id == "BOT" {
                continue;
            }
            let Ok(steam_id) = SteamID::from_steam3(&info.steam_id) else {
                continue;
            };
            handled_steam_ids.push(steam_id);

            let angle_history = self.angle_history.entry(steam_id).or_default();

            // TODO: get some more robust way of filtering out demo jank
            let reset_angle_history_because_of_jank = self.jg.teleported(&steam_id, ticknum) <= 60 || self.jg.spawned(&steam_id, ticknum) <= 60;
            if reset_angle_history_because_of_jank {
                angle_history.reset();
            }

            let angle_new = player.into();
            let Some(angle_acceleration) = angle_history.insert_angle(angle_new) else {
                continue;
            };

            let acceleration_magnitude = angle_acceleration.0.magnitude();
            let ticks_since_firing = self.jg.fired(&steam_id, ticknum);
            let trigger_detection = acceleration_magnitude >= threshold && ticks_since_firing <= max_fire_delay;
            if trigger_detection {
                let mut detection_data_map = serde_json::Map::new();
                detection_data_map.insert("view_angle".into(), angle_new.into());
                detection_data_map.insert("acceleration".into(), angle_acceleration.into());
                let detection_data = serde_json::Value::Object(detection_data_map);
                let detection = Detection {
                    tick: ticknum,
                    algorithm: self.algorithm_name().to_string(),
                    player: steam_id,
                    data: detection_data,
                };
                detections.push(detection);
            }
        }

        self.angle_history
            .iter_mut()
            .filter(|(id, _)| !handled_steam_ids.contains(id))
            .for_each(|(_, history)| history.reset());

        self.detections.extend(detections.clone());

        Ok(detections)
    }

    fn on_message(
        &mut self,
        message: &tf_demo_parser::demo::message::Message,
        state: &crate::base::cheat_analyser_base::CheatAnalyserState,
        parser_state: &tf_demo_parser::ParserState,
        tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<crate::lib::algorithm::Detection>, anyhow::Error> {
        self.jg.on_message(message, state, parser_state, tick);
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Detection>, anyhow::Error> {
        Ok(self.detections.clone())
    }

    fn params(&mut self) -> Option<&mut crate::lib::parameters::Parameters> {
        Some(&mut self.parameters)
    }
}
