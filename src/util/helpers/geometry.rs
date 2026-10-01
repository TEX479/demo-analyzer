use crate::base::cheat_analyser_base::Player;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    ops::{Add, Neg, Sub},
};

// TODO: add example
/// Compute the difference in viewangles. We have to account for the fact viewangles are in a circle.
/// E.g. If viewangle goes from 350 to 10 degrees, we want to return 20 degrees.
///
/// The returned values can be thought of the offset the player had to move their mouse to.
///
/// This does *not* equal the angular difference, since when looking up/down, the angular difference decreases compared to the mouse movement
pub fn viewangle_delta(current_angle: ViewAngle, previous_angle: ViewAngle, tick_delta: u32) -> (f32, f32) {
    let tick_delta = tick_delta.max(1);
    let va_delta = {
        let diff = (current_angle.yaw - previous_angle.yaw).rem_euclid(360.0);
        if diff > 180.0 { diff - 360.0 } else { diff }
    } / tick_delta as f32;
    let pa_delta = (current_angle.pitch - previous_angle.pitch) / tick_delta as f32;
    (va_delta, pa_delta)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// a player's view angle
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewAngle {
    /// yaw aka `Player.view_angle` in degrees
    pub yaw: f32,
    /// pitch aka `Player.pitch_angle` in degrees
    pub pitch: f32,
}

impl ViewAngle {
    // TODO: add example
    /// calculate the normalized `Vec3` that points in `ViewAngle`
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::{ViewAngle, Vec3};
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// fn vecs_equal(v1: Vec3, v2: Vec3) -> bool {
    ///     floats_are_equal(v1.x, v2.x)
    ///     && floats_are_equal(v1.y, v2.y)
    ///     && floats_are_equal(v1.z, v2.z)
    /// }
    ///
    /// let v1 = ViewAngle { yaw: 0.0, pitch: 0.0 }.to_unit_vec();
    /// let v2 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// assert!(vecs_equal(v1, v2));
    ///
    /// let v1 = ViewAngle { yaw: 90.0, pitch: 0.0 }.to_unit_vec();
    /// let v2 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    /// assert!(vecs_equal(v1, v2));
    ///
    /// let v1 = ViewAngle { yaw: 0.0, pitch: 90.0 }.to_unit_vec();
    /// let v2 = Vec3 { x: 0.0, y: 0.0, z: -1.0 };
    /// assert!(vecs_equal(v1, v2));
    /// ```
    pub fn to_unit_vec(self) -> Vec3 {
        let Self { yaw, pitch } = self;

        let yaw = yaw.to_radians();
        let pitch = pitch.to_radians();
        Vec3 {
            x: pitch.cos() * yaw.cos(),
            y: pitch.cos() * yaw.sin(),
            z: -pitch.sin(),
        }
    }

    /// Compute the total angular difference between two angles in degrees
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngle;
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    ///
    /// let x = ViewAngle { yaw: 0.0, pitch: 0.0 };
    /// let y = ViewAngle { yaw: 90.0, pitch: 27.3 };
    /// let delta = x.delta(y);
    /// let expected_delta = 90.0;
    /// assert!(-EPSILON < delta - expected_delta && delta - expected_delta < EPSILON);
    ///
    ///
    /// let x = ViewAngle { yaw: 0.0, pitch: 85.0 };
    /// let y = ViewAngle { yaw: 90.0, pitch: 85.0 };
    /// let delta = x.delta(y);
    /// let expected_delta = 7.0665;
    /// println!("delta: {}", delta);
    /// assert!(-EPSILON < delta - expected_delta && delta - expected_delta < EPSILON);
    /// ```
    pub fn delta(self, other: Self) -> f32 {
        let vec1 = self.to_unit_vec();
        let vec2 = other.to_unit_vec();

        let dot = vec1.dot(vec2);
        dot.acos().to_degrees()
    }
}

impl Vec3 {
    // TODO: add example
    /// performs the dot product with rhs
    #[inline(always)]
    pub const fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    /// returns the length of the vector
    ///
    /// If you need the square of the length, use `Vec3::length_square`, as this method takes it's squareroot
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    ///
    /// let x = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// let length = x.length();
    /// let expected_length = 1.0;
    /// assert!(-EPSILON < length - expected_length && length - expected_length < EPSILON);
    ///
    ///
    /// let x = Vec3 { x: 1.0, y: 1.0, z: 1.0 };
    /// let length = x.length();
    /// let expected_length = 3.0_f32.sqrt();
    /// assert!(-EPSILON < length - expected_length && length - expected_length < EPSILON);
    /// ```
    #[inline(always)]
    pub fn length(self) -> f32 {
        self.length_square().sqrt()
    }

    /// returns the square of the length of the vector
    ///
    /// This is faster than `Vec3::length()`, since it does not take the squareroot.
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    ///
    /// let x = Vec3 {x: 1.0, y: 0.0, z: 0.0};
    /// let length = x.length_square();
    /// let expected_length = 1.0;
    /// assert!(-EPSILON < length - expected_length && length - expected_length < EPSILON);
    ///
    ///
    /// let x = Vec3 {x: 1.0, y: 2.0, z: 3.0};
    /// let length = x.length_square();
    /// let expected_length = 14.0;
    /// assert!(-EPSILON < length - expected_length && length - expected_length < EPSILON);
    /// ```
    #[inline(always)]
    pub const fn length_square(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self {
            x: self.x.neg(),
            y: self.y.neg(),
            z: self.z.neg(),
        }
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
impl From<Vec3> for tf_demo_parser::demo::vector::Vector {
    fn from(value: Vec3) -> Self {
        let Vec3 { x, y, z } = value;
        Self { x, y, z }
    }
}

impl TryFrom<Vec3> for ViewAngle {
    // TODO: get some real error type
    type Error = ();

    fn try_from(value: Vec3) -> Result<Self, Self::Error> {
        // TODO: implement
        todo!()
    }
}
