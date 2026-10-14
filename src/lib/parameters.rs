use std::{collections::HashMap, fmt::Display};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum AlgorithmParameter<'a> {
    Float32(&'a mut f32),
    Int32(&'a mut i32),
    Bool(&'a mut bool),
    Usize(&'a mut usize),
    Unsigned32(&'a mut u32),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SerializableParameter {
    Float32(f32),
    Int32(i32),
    Bool(bool),
    Usize(usize),
    Unsigned32(u32),
}

#[derive(Debug)]
pub enum ParameterError {
    TypeMismatch,
}

impl Display for ParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type mismatch")
    }
}

macro_rules! impl_display {
    ($($variant:ident),+) => {
        impl Display for SerializableParameter {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant(v) => write!(f, "{v}"),)+
                }
            }
        }
        impl<'a> Display for AlgorithmParameter<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant(v) => write!(f, "{v}"),)+
                }
            }
        }
    };
}

impl_display!(Float32, Int32, Bool, Usize, Unsigned32);

impl std::error::Error for ParameterError {}

pub type AlgorithmParameters<'a> = HashMap<String, AlgorithmParameter<'a>>;

// Maps parameter names to their values.
pub type SerializableParameters = HashMap<String, SerializableParameter>;

// Maps algorithm names to their parameters.
pub type Config = HashMap<String, SerializableParameters>;

impl<'a> AlgorithmParameter<'a> {
    /// tries to modify the internally saved value
    ///
    /// This can be used to try and set the parameters of an algorithm to values loaded from a config.
    pub fn try_modify_value(&'a mut self, set_to: SerializableParameter) -> Result<(), ParameterError> {
        match self {
            Self::Float32(v) => {
                **v = set_to.try_into()?;
            }
            Self::Int32(v) => {
                **v = set_to.try_into()?;
            }
            Self::Bool(v) => {
                **v = set_to.try_into()?;
            }
            Self::Usize(v) => {
                **v = set_to.try_into()?;
            }
            Self::Unsigned32(v) => {
                **v = set_to.try_into()?;
            }
        }
        Ok(())
    }
}

pub fn get_parameter_value<T>(params: &SerializableParameters, param_name: &str) -> anyhow::Result<T>
where
    T: for<'a> TryFrom<&'a SerializableParameter, Error = ParameterError>,
{
    params
        .get(param_name)
        .context(format!("Parameter {} not found", param_name))?
        .try_into()
        .context(format!("Parameter {} could not be converted to the expected type.", param_name))
}

impl<'a> From<AlgorithmParameter<'a>> for SerializableParameter {
    fn from(value: AlgorithmParameter<'a>) -> Self {
        match value {
            AlgorithmParameter::Float32(v) => Self::Float32(*v),
            AlgorithmParameter::Int32(v) => Self::Int32(*v),
            AlgorithmParameter::Bool(v) => Self::Bool(*v),
            AlgorithmParameter::Usize(v) => Self::Usize(*v),
            AlgorithmParameter::Unsigned32(v) => Self::Unsigned32(*v),
        }
    }
}

/// updates `AlgorithmParameters` based on `SerializableParameters`
///
/// This uses `tracing` log any changes and errors
pub fn try_update_algorithm_params<'a>(algo_params: &'a mut AlgorithmParameters<'a>, update_with: &SerializableParameters) {
    for (name, value) in algo_params.iter_mut() {
        let Some(update_value) = update_with.get(name) else {
            tracing::info!("{} = {} (default)", name, value);
            continue;
        };
        match value.try_modify_value(*update_value) {
            Ok(()) => {
                // TODO: replace `update_value` with `value`
                tracing::info!("{} = {} (changed)", name, update_value)
            }
            Err(e) => {
                tracing::error!("could not update {:?} with {:?}: {:?}", name, update_value, e);
            }
        };
    }
}

macro_rules! impl_from_both_ways {
    ($from_type:ty, $enum_variant:ident, $type_mismatch_type:ty, $type_mismatch_expr:expr) => {
        impl From<$from_type> for SerializableParameter {
            fn from(value: $from_type) -> Self {
                Self::$enum_variant(value)
            }
        }
        impl TryFrom<SerializableParameter> for $from_type {
            type Error = $type_mismatch_type;
            fn try_from(value: SerializableParameter) -> Result<$from_type, $type_mismatch_type> {
                match value {
                    SerializableParameter::$enum_variant(v) => Ok(v),
                    _ => Err($type_mismatch_expr),
                }
            }
        }
        impl<'a> From<&'a mut $from_type> for AlgorithmParameter<'a> {
            fn from(value: &'a mut $from_type) -> Self {
                Self::$enum_variant(value)
            }
        }
        impl<'a> TryFrom<AlgorithmParameter<'a>> for &'a mut $from_type {
            type Error = $type_mismatch_type;
            fn try_from(value: AlgorithmParameter<'a>) -> Result<&'a mut $from_type, $type_mismatch_type> {
                match value {
                    AlgorithmParameter::$enum_variant(v) => Ok(v),
                    _ => Err($type_mismatch_expr),
                }
            }
        }
    };

    ($type_mismatch_type:ty, $type_mismatch_expr:expr, [$(($from_type:ty, $enum_variant:ident)),+]) => {$(
        impl_from_both_ways!($from_type, $enum_variant, $type_mismatch_type, $type_mismatch_expr);
    )+};
}

/*
macro_rules! impl_from_both_ways_with_lifetimes {
    ($from_type:ty, $enum_name:tt, $enum_variant:ident, $type_mismatch_type:ty, $type_mismatch_expr:expr) => {
        impl<'a> From<$from_type> for $enum_name<'a>
        where
            $from_type: 'a
        {
            fn from(value: $from_type) -> Self {
                Self::$enum_variant(value)
            }
        }
        impl<'a, 'b> TryFrom<$enum_name<'a>> for $from_type
        where
            $from_type: 'b,
            'a: 'b
        {
            type Error = $type_mismatch_type;
            fn try_from(value: $enum_name) -> Result<$from_type, $type_mismatch_type> {
                match value {
                    $enum_name::$enum_variant(v) => Ok(v),
                    _ => Err($type_mismatch_expr),
                }
            }
        }
    };

    ($enum_name:tt, $type_mismatch_type:ty, $type_mismatch_expr:expr, [$(($from_type:ty, $enum_variant:ident)),+]) => {$(
        impl_from_both_ways_with_lifetimes!($from_type, $enum_name, $enum_variant, $type_mismatch_type, $type_mismatch_expr);
    )+};
}
// */
impl_from_both_ways!(
    ParameterError,
    ParameterError::TypeMismatch,
    [(i32, Int32), (f32, Float32), (bool, Bool), (usize, Usize), (u32, Unsigned32)]
);

#[macro_export]
macro_rules! make_algorithm_params {
    ($base_struct:expr; [$($name:ident),+ $(,)?] $(,)?) => {{
        let mut params = std::collections::HashMap::new();
        $(
            params.insert(stringify!($name).to_owned(), (&mut $base_struct.$name).into());
        )+
        params
    }};
}
