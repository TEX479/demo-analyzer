use crate::base::cheat_analyser_base::Player;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    ops::{Add, Neg, Sub},
};

pub static F32_EPSILON: f32 = 0.00001;

/// Compute the difference in viewangles. We have to account for the fact viewangles are in a circle.
/// E.g. If viewangle goes from 350 to 10 degrees, we want to return 20 degrees.
/// This function takes the tick-delta into account.
///
/// The returned values can be thought of the offset the player had to move their mouse to.
///
/// This does *not* equal the angular difference, since when looking up/down, the angular difference decreases compared to the mouse movement
///
/// # Example
/// ```
/// # use analysis_template::util::helpers::geometry::{viewangle_delta, ViewAngle};
/// let va1 = ViewAngle { yaw: 0.0, pitch: 0.0 };
/// let va2 = ViewAngle { yaw: 90.0, pitch: 0.0 };
/// let (delta_yaw, delta_pitch) = viewangle_delta(va1, va2, 1);
/// assert!(floats_are_equal(delta_yaw, -90.0));
/// assert!(floats_are_equal(delta_pitch, 0.0));
///
/// let va1 = ViewAngle { yaw: 0.0, pitch: 0.0 };
/// let va2 = ViewAngle { yaw: 90.0, pitch: 0.0 };
/// let (delta_yaw, delta_pitch) = viewangle_delta(va1, va2, 2);
/// assert!(floats_are_equal(delta_yaw, -45.0));
/// assert!(floats_are_equal(delta_pitch, 0.0));
///
/// // in this case, the actual world angles are very close
/// // this demonstrates the difference to `ViewAngle::delta`
/// let va1 = ViewAngle { yaw: 0.0, pitch: 85.0 };
/// let va2 = ViewAngle { yaw: 90.0, pitch: 89.0 };
/// let (delta_yaw, delta_pitch) = viewangle_delta(va1, va2, 1);
/// assert!(floats_are_equal(delta_yaw, -90.0));
/// assert!(floats_are_equal(delta_pitch, -4.0));
///
///
/// // since floating point operations have rounding, compare using some epsilon
/// const EPSILON: f32 = 0.001;
/// fn floats_are_equal(f1: f32, f2: f32) -> bool {
///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
/// }
/// ```
// TODO: move to `ViewAngle`s & maybe remove `tick_delta` parameter
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewAngleDelta {
    pub yaw: f32,
    pub pitch: f32,
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
    /// calculate the normalized `Vec3` that points in `ViewAngle`
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::{ViewAngle, Vec3};
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
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn vecs_equal(v1: Vec3, v2: Vec3) -> bool {
    ///     floats_are_equal(v1.x, v2.x)
    ///     && floats_are_equal(v1.y, v2.y)
    ///     && floats_are_equal(v1.z, v2.z)
    /// }
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
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

    /// Compute the difference in viewangles. We have to account for the fact viewangles are in a circle.
    /// E.g. If viewangle goes from 350 to 10 degrees, we want to return 20 degrees.
    ///
    /// The returned values can be thought of the offset the player had to move their mouse to.
    ///
    /// This does *not* equal the angular difference, since when looking up/down, the angular difference decreases compared to the mouse movement
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::{viewangle_delta, ViewAngle, ViewAngleDelta};
    ///
    /// let va1 = ViewAngle { yaw: 0.0, pitch: 0.0 };
    /// let va2 = ViewAngle { yaw: 90.0, pitch: 0.0 };
    /// let ViewAngleDelta { yaw, pitch } = va1.viewangle_delta(va2);
    /// assert!(floats_are_equal(yaw, 90.0));
    /// assert!(floats_are_equal(pitch, 0.0));
    ///
    /// let va1 = ViewAngle { yaw: 0.0, pitch: 0.0 };
    /// let va2 = ViewAngle { yaw: -5.0, pitch: 0.0 };
    /// let ViewAngleDelta { yaw, pitch } = va1.viewangle_delta(va2);
    /// assert!(floats_are_equal(yaw, -5.0));
    /// assert!(floats_are_equal(pitch, 0.0));
    ///
    /// // in this case, the actual world angles are very close
    /// // this demonstrates the difference to `ViewAngle::delta`
    /// let va1 = ViewAngle { yaw: 0.0, pitch: 85.0 };
    /// let va2 = ViewAngle { yaw: 90.0, pitch: 89.0 };
    /// let ViewAngleDelta { yaw, pitch } = va1.viewangle_delta(va2);
    /// assert!(floats_are_equal(yaw, 90.0));
    /// assert!(floats_are_equal(pitch, 4.0));
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```    
    pub fn viewangle_delta(self, next: Self) -> ViewAngleDelta {
        let yaw = {
            let diff = (next.yaw - self.yaw).rem_euclid(360.0);
            if diff > 180.0 { diff - 360.0 } else { diff }
        };
        let pitch = next.pitch - self.pitch;
        ViewAngleDelta { yaw, pitch }
    }

    /// Compute the total angular difference between two angles in degrees
    ///
    /// This computes the actual in-world difference angle,
    /// meaning that having a pitch of 90° and changing yaw does not affect the delta output.
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngle;
    /// let x = ViewAngle { yaw: 0.0, pitch: 0.0 };
    /// let y = ViewAngle { yaw: 90.0, pitch: 27.3 };
    /// let delta = x.world_angle_delta(y);
    /// let expected_delta = 90.0;
    /// assert!(floats_are_equal(delta, expected_delta));
    ///
    ///
    /// let x = ViewAngle { yaw: 0.0, pitch: 85.0 };
    /// let y = ViewAngle { yaw: 90.0, pitch: 85.0 };
    /// let delta = x.world_angle_delta(y);
    /// let expected_delta = 7.0665;
    /// println!("delta: {}", delta);
    /// assert!(floats_are_equal(delta, expected_delta));
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```
    pub fn world_angle_delta(self, other: Self) -> f32 {
        let vec1 = self.to_unit_vec();
        let vec2 = other.to_unit_vec();

        let dot = vec1.dot(vec2).clamp(-1.0, 1.0);
        dot.acos().to_degrees()
    }

    /// checks if a field is a NaN
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngle;
    /// assert!(!ViewAngle { yaw: 0.0, pitch: 0.0}.is_nan());
    /// assert!(ViewAngle { yaw: f32::NAN, pitch: 0.0}.is_nan());
    /// assert!(ViewAngle { yaw: 0.0, pitch: f32::NAN}.is_nan());
    /// assert!(ViewAngle { yaw: f32::NAN, pitch: f32::NAN}.is_nan());
    /// ```
    pub fn is_nan(&self) -> bool {
        self.yaw.is_nan() || self.pitch.is_nan()
    }
}

impl Vec3 {
    /// performs the dot product with rhs
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    ///
    /// let v1 = Vec3 { x: 1.0, y: 2.0, z: 3.0 };
    /// let v2 = Vec3 { x: 0.0, y: 0.0, z: 2.0 };
    /// assert_eq!(v1.dot(v2), 6.0);
    ///
    /// let v1 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: 0.0, y: 0.0, z: 2.0 };
    /// assert_eq!(v1.dot(v2), 0.0);
    /// ```
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
    /// let x = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// let length = x.length();
    /// let expected_length = 1.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// let x = Vec3 { x: 1.0, y: 1.0, z: 1.0 };
    /// let length = x.length();
    /// let expected_length = 3.0_f32.sqrt();
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
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
    /// let x = Vec3 {x: 1.0, y: 0.0, z: 0.0};
    /// let length = x.length_square();
    /// let expected_length = 1.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// let x = Vec3 {x: 1.0, y: 2.0, z: 3.0};
    /// let length = x.length_square();
    /// let expected_length = 14.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```
    #[inline(always)]
    pub const fn length_square(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// returns the normalized vector
    ///
    /// `normalized` in this case means "a vector which has a length of 1.0"
    ///
    /// # Division By Zero
    /// This method devides by the length of the vector. If the vector's length is zero,
    /// this can result in undefined behavior, `f32::NAN`s and other weird numbers.
    /// zero-checking falls onto the caller
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// let v1 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// assert_eq!(v1.normalized(), v2);
    ///
    /// let v1 = Vec3 { x: 12.0, y: 15.0, z: 16.0 };
    /// let v2 = Vec3 { x: 0.48, y: 0.6, z: 0.64 };
    /// assert!(vecs_equal(v1.normalized(), v2));
    ///
    /// let v1 = Vec3 { x: 0.0, y: 0.0, z: 0.0 }.normalized();
    /// assert!(v1.x.is_nan());
    /// assert!(v1.y.is_nan());
    /// assert!(v1.z.is_nan());
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.00001;
    /// fn vecs_equal(v1: Vec3, v2: Vec3) -> bool {
    ///     floats_are_equal(v1.x, v2.x)
    ///     && floats_are_equal(v1.y, v2.y)
    ///     && floats_are_equal(v1.z, v2.z)
    /// }
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```
    pub fn normalized(self) -> Self {
        self.scaled(1.0 / self.length())
    }

    /// performs the cross product
    ///
    /// (wikipedia.org/wiki/Cross_product)
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// let v1 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    /// let v3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 };
    /// assert_eq!(v1.cross_product(v2), v3);
    ///
    /// let v1 = Vec3 { x: 0.0, y: 0.0, z: 2.0 };
    /// let v2 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    /// let v3 = Vec3 { x: -2.0, y: 0.0, z: 0.0 };
    /// assert_eq!(v1.cross_product(v2), v3);
    /// ```
    pub fn cross_product(self, rhs: Self) -> Self {
        let x = self.y * rhs.z - self.z * rhs.y;
        let y = self.z * rhs.x - self.x * rhs.z;
        let z = self.x * rhs.y - self.y * rhs.x;
        Self { x, y, z }
    }

    /// scales the vector based on a multiplier
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// let v1 = Vec3 { x: 1.0, y: 2.0, z: 3.0 }.scaled(2.0);
    /// let v2 = Vec3 { x: 2.0, y: 4.0, z: 6.0 };
    /// assert_eq!(v1, v2);
    /// ```
    #[inline(always)]
    pub fn scaled(self, multiplier: f32) -> Self {
        Self {
            x: self.x * multiplier,
            y: self.y * multiplier,
            z: self.z * multiplier,
        }
    }

    /// checks if a field of self is NaN
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// assert!(!Vec3 { x: 0.0, y: 0.0, z: 0.0 }.is_nan());
    /// assert!(Vec3 { x: f32::NAN, y: 0.0, z: 0.0 }.is_nan());
    /// assert!(Vec3 { x: 0.0, y: f32::NAN, z: 0.0 }.is_nan());
    /// assert!(Vec3 { x: f32::NAN, y: f32::NAN, z: 0.0 }.is_nan());
    /// assert!(Vec3 { x: 0.0, y: 0.0, z: f32::NAN }.is_nan());
    /// assert!(Vec3 { x: f32::NAN, y: 0.0, z: f32::NAN }.is_nan());
    /// assert!(Vec3 { x: 0.0, y: f32::NAN, z: f32::NAN }.is_nan());
    /// assert!(Vec3 { x: f32::NAN, y: f32::NAN, z: f32::NAN }.is_nan());
    /// ```
    pub fn is_nan(&self) -> bool {
        self.x.is_nan() || self.y.is_nan() || self.z.is_nan()
    }
}

impl ViewAngleDelta {
    /// returns the magnitude of the delta
    ///
    /// If you need the square of the delta, use `ViewAngleDelta::magnitude_squared`, as this method takes it's squareroot
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngleDelta;
    ///
    /// let x = ViewAngleDelta { yaw: 1.0, pitch: 0.0 };
    /// let length = x.magnitude();
    /// let expected_length = 1.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    /// let x = ViewAngleDelta { yaw: 3.0, pitch: 4.0 };
    /// let length = x.magnitude();
    /// let expected_length = 5.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```
    pub fn magnitude(self) -> f32 {
        self.magnitude_squared().sqrt()
    }

    /// returns the square of the magnitude of the vector
    ///
    /// This is faster than `ViewAngleDelta::magnitude()`, since it does not take the squareroot.
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngleDelta;
    /// let x = ViewAngleDelta { yaw: 1.0, pitch: 0.0 };
    /// let length = x.magnitude_squared();
    /// let expected_length = 1.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// let x = ViewAngleDelta { yaw: 3.0, pitch: 4.0 };
    /// let length = x.magnitude_squared();
    /// let expected_length = 25.0;
    /// assert!(floats_are_equal(length, expected_length));
    ///
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```
    pub const fn magnitude_squared(self) -> f32 {
        self.yaw * self.yaw + self.pitch * self.pitch
    }

    /// checks if a field of self is NaN
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngleDelta;
    /// assert!(!ViewAngleDelta { yaw: 0.0, pitch: 0.0 }.is_nan());
    /// assert!(ViewAngleDelta { yaw: f32::NAN, pitch: 0.0 }.is_nan());
    /// assert!(ViewAngleDelta { yaw: 0.0, pitch: f32::NAN }.is_nan());
    /// assert!(ViewAngleDelta { yaw: f32::NAN, pitch: f32::NAN }.is_nan());
    /// ```
    pub fn is_nan(&self) -> bool {
        self.yaw.is_nan() || self.pitch.is_nan()
    }
}

// TODO: add unit tests for Vec3 & ViewAngleDelta arithmetic
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
