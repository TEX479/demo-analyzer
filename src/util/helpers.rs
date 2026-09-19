// Put individual functions with widespread utility in here.
// For more complicated structures, consider making your own file in the /util directory.

use crate::base::cheat_analyser_base::Player;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ViewAngle {
    /// yaw aka `Player.view_angle`
    pub yaw: f32,
    /// pitch aka `Player.pitch_angle`
    pub pitch: f32,
}

impl ViewAngle {
    // TODO: wrap the 3 floats in a `Vec3` struct
    pub fn to_unit_vec(self) -> (f32, f32, f32) {
        let Self { yaw, pitch } = self;

        let yaw = yaw.to_radians();
        let pitch = pitch.to_radians();
        (pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos())
    }

    /// Compute the total angular difference between two angles
    pub fn delta(self, other: Self) -> f32 {
        let vec1 = self.to_unit_vec();
        let vec2 = other.to_unit_vec();

        let dot = vec1.0 * vec2.0 + vec1.1 * vec2.1 + vec1.2 * vec2.2;
        dot.acos().to_degrees()
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
