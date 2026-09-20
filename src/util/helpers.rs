// Put individual functions with widespread utility in here.
// For more complicated structures, consider making your own file in the /util directory.

use crate::base::cheat_analyser_base::{Player, PlayerState};
use serde::{Deserialize, Serialize};
use steamid_ng::SteamID;
use tf_demo_parser::demo::message::packetentities::EntityId;

// Compute the difference in viewangles. We have to account for the fact viewangles are in a circle.
// E.g. If viewangle goes from 350 to 10 degrees, we want to return 20 degrees.
pub fn viewangle_delta(current_angle: ViewAngle, previous_angle: ViewAngle, tick_delta: u32) -> (f32, f32) {
    let tick_delta = if tick_delta < 1 { 1 } else { tick_delta };
    let va_delta = {
        let diff = (current_angle.yaw - previous_angle.yaw).rem_euclid(360.0);
        if diff > 180.0 { diff - 360.0 } else { diff }
    } / tick_delta as f32;
    let pa_delta = (current_angle.pitch - previous_angle.pitch) / tick_delta as f32;
    (va_delta, pa_delta)
}

// Since TF2 has an object limit of 2048, the lowest 11 bits of the handle ID represent the entity ID.
// Source: https://developer.valvesoftware.com/wiki/CHandle
#[allow(dead_code)]
pub fn handle_to_entid(handle: u32) -> EntityId {
    let entid = handle & 0x7FF;
    EntityId::from(entid)
}

#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ViewAngle {
    /// yaw aka `Player.view_angle`
    pub yaw: f32,
    /// pitch aka `Player.pitch_angle`
    pub pitch: f32,
}

impl ViewAngle {
    /// calculate the normalized `Vec3` that points in `ViewAngle`
    pub fn to_unit_vec(self) -> Vec3 {
        let Self { yaw, pitch } = self;

        let yaw = yaw.to_radians();
        let pitch = pitch.to_radians();
        Vec3 {
            x: pitch.cos() * yaw.cos(),
            y: pitch.sin() * yaw.sin(),
            z: -pitch.sin(),
        }
    }

    /// Compute the total angular difference between two angles in degrees
    pub fn delta(self, other: Self) -> f32 {
        let vec1 = self.to_unit_vec();
        let vec2 = other.to_unit_vec();

        let dot = vec1.dot(vec2);
        dot.acos().to_degrees()
    }
}

impl Vec3 {
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
}

impl From<&Player> for ViewAngle {
    fn from(value: &Player) -> Self {
        Self {
            yaw: value.view_angle,
            pitch: value.pitch_angle,
        }
    }
}

impl From<tf_demo_parser::demo::vector::Vector> for Vec3 {
    fn from(value: tf_demo_parser::demo::vector::Vector) -> Self {
        let tf_demo_parser::demo::vector::Vector { x, y, z } = value;
        Self { x, y, z }
    }
}

pub fn valid_players<'a>(
    players: impl Iterator<Item = &'a Player>,
) -> impl Iterator<Item = (&'a Player, &'a tf_demo_parser::demo::parser::analyser::UserInfo, SteamID)> {
    players
        .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().map(|i| (p, i)))
        .filter_map(|(p, i)| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id)))
}

pub fn valid_player<'a>(
    players: impl Iterator<Item = &'a Player>,
    steam_id: SteamID,
) -> Option<(&'a Player, &'a tf_demo_parser::demo::parser::analyser::UserInfo)> {
    players
        .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().and_then(|i| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id))))
        .find(|(_, _, id)| *id == steam_id)
        .map(|(p, i, _)| (p, i))
}
