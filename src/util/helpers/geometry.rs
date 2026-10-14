use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    ops::{Add, Div, Mul, Neg, Sub},
};

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
    /// # use analysis_template::util::helpers::geometry::{ViewAngle, ViewAngleDelta};
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
        self.dot(self)
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
        self / self.length()
    }

    /// returns the normalized vector
    ///
    /// `normalized` in this case means "a vector which has a length of 1.0"
    ///
    /// # Division By Zero
    /// This method devides by the length of the vector. If the vector's length is zero
    /// or `f32::NAN`s appear anywhere within the output, `None` is returned instead.
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// let v1 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    /// assert_eq!(v1.checked_normalized(), Some(v2));
    ///
    /// let v1 = Vec3 { x: 12.0, y: 15.0, z: 16.0 };
    /// let v2 = Vec3 { x: 0.48, y: 0.6, z: 0.64 };
    /// assert!(vecs_equal(v1.checked_normalized().expect("v1 is non-zero and non-NaN and thus has to be valid"), v2));
    ///
    /// let v = Vec3 { x: 0.0, y: 0.0, z: 0.0 }.normalized();
    /// assert_eq!(v.checked_normalized(), None);
    ///
    /// let v = Vec3 { x: 1.0, y: 0.0, z: f32::NAN };
    /// assert_eq!(v.checked_normalized(), None);
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
    pub fn checked_normalized(self) -> Option<Self> {
        let v = self.normalized();
        if v.is_nan() { None } else { Some(v) }
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

    // TODO: test more
    /// gets the angle between self and rhs
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::Vec3;
    /// let v1 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    /// assert_eq!(v1.angle_between(v2), 0.0);
    ///
    /// let v1 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: -2.0, y: 0.0, z: 0.0 };
    /// assert!(floats_are_equal(v1.angle_between(v2).abs(), 180.0));
    ///
    /// let v1 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    /// let v2 = Vec3 { x: 0.0, y: 2.0, z: 0.0 };
    /// assert!(floats_are_equal(v1.angle_between(v2).abs(), 90.0));
    ///
    /// // since floating point operations have rounding, compare using some epsilon
    /// const EPSILON: f32 = 0.00001;
    /// fn floats_are_equal(f1: f32, f2: f32) -> bool {
    ///     -EPSILON < f1 - f2 && f1 - f2 < EPSILON
    /// }
    /// ```
    pub fn angle_between(self, rhs: Self) -> f32 {
        (self.dot(rhs) / (self.length_square() * rhs.length_square()).sqrt())
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
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
        self.dot(self)
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

    /// performs the dot product with rhs
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::ViewAngleDelta;
    ///
    /// let v1 = ViewAngleDelta { yaw: 2.0, pitch: 3.0 };
    /// let v2 = ViewAngleDelta { yaw: 0.0, pitch: 4.0 };
    /// assert_eq!(v1.dot(v2), 12.0);
    ///
    /// let v1 = ViewAngleDelta { yaw: 1.0, pitch: 0.0 };
    /// let v2 = ViewAngleDelta { yaw: 0.0, pitch: 2.0 };
    /// assert_eq!(v1.dot(v2), 0.0);
    /// ```
    pub const fn dot(self, rhs: ViewAngleDelta) -> f32 {
        self.yaw * rhs.yaw + self.pitch * rhs.pitch
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
impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        self.scaled(rhs)
    }
}
impl Div<f32> for Vec3 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

impl Add for ViewAngleDelta {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            yaw: self.yaw + rhs.yaw,
            pitch: self.pitch + rhs.pitch,
        }
    }
}
impl Sub for ViewAngleDelta {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            yaw: self.yaw - rhs.yaw,
            pitch: self.pitch - rhs.pitch,
        }
    }
}
impl Neg for ViewAngleDelta {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self {
            yaw: self.yaw.neg(),
            pitch: self.pitch.neg(),
        }
    }
}
impl Mul<f32> for ViewAngleDelta {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            yaw: self.yaw * rhs,
            pitch: self.pitch * rhs,
        }
    }
}
impl Div<f32> for ViewAngleDelta {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self {
            yaw: self.yaw / rhs,
            pitch: self.pitch / rhs,
        }
    }
}

impl Add<ViewAngleDelta> for ViewAngle {
    type Output = Self;
    fn add(self, rhs: ViewAngleDelta) -> Self::Output {
        let mut yaw = self.yaw + rhs.yaw;
        let mut pitch = (self.pitch + rhs.pitch).rem_euclid(360.0);
        pitch = if pitch > 180.0 { pitch - 360.0 } else { pitch };
        if pitch > 90.0 {
            pitch -= 90.0;
            yaw += 180.0;
        } else if pitch < -90.0 {
            pitch += 90.0;
            yaw += 180.0;
        }
        yaw = yaw.rem_euclid(360.0);
        Self {
            yaw: if yaw > 180.0 { yaw - 360.0 } else { yaw },
            pitch,
        }
    }
}
impl Sub<ViewAngleDelta> for ViewAngle {
    type Output = Self;
    fn sub(self, rhs: ViewAngleDelta) -> Self::Output {
        self + (-rhs)
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

impl From<Vec3> for ViewAngle {
    /// gets the viewangle along the provided `Vec3`
    ///
    /// if the vector is of length 0, the return value is the default `0,0`
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::geometry::{Vec3, ViewAngle};
    ///
    /// let vector = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    /// let expected_viewangle = ViewAngle { yaw: 0.0, pitch: 0.0 };
    /// assert_eq!(ViewAngle::from(vector), expected_viewangle);
    ///
    /// let vector = Vec3 { x: 1.0, y: 1.0, z: 2_f32.sqrt() };
    /// let expected_viewangle = ViewAngle { yaw: 45.0, pitch: -45.0 };
    /// assert_eq!(ViewAngle::from(vector), expected_viewangle);
    /// ```
    fn from(value: Vec3) -> Self {
        // This code is basically a untested copy from `fidoo/psilent4.rs`

        let horizontal = (value.x * value.x + value.y * value.y).sqrt();
        Self {
            yaw: value.y.atan2(value.x).to_degrees(),
            pitch: (-value.z).atan2(horizontal).to_degrees(),
        }
    }
}

impl TryFrom<&tf_demo_parser::demo::sendprop::SendPropValue> for Vec3 {
    type Error = tf_demo_parser::demo::parser::MalformedSendPropDefinitionError;
    fn try_from(value: &tf_demo_parser::demo::sendprop::SendPropValue) -> Result<Self, Self::Error> {
        tf_demo_parser::demo::vector::Vector::try_from(value).map(Vec3::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_angles_roundtrip() {
        let dir = ViewAngle { yaw: 45.0, pitch: 15.0 }.to_unit_vec();
        let ViewAngle { yaw, pitch } = dir.into();
        assert!((yaw - 45.0).abs() < 0.01);
        assert!((pitch - 15.0).abs() < 0.01);
    }
}
