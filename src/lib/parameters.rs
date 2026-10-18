use std::{collections::HashMap, fmt::Display};

use anyhow::Context;
use serde::{Deserialize, Serialize};

// defines all enum variants
// used in giant_maker_macro
macro_rules! define_enums {
    ($(($variant:ident, $inner_type:ty)),+ $(,)?) => {
        /// an enum that holds mutable references to parameters of an algorithm
        ///
        /// can not be serialized or cloned. use `SerializableParameter` for that
        #[derive(Debug)]
        pub enum AlgorithmParameter<'a> {$(
            $variant(&'a mut $inner_type),
        )+}

        /// an enum equivalent to `AlgorithmParameter` but with owned data
        ///
        /// This can be serialized, cloned, etc. because the data is owned.
        #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
        pub enum SerializableParameter {$(
            $variant($inner_type),
        )+}
    };
}

/// Maps parameter names to an algorithm's values
pub type AlgorithmParameters<'a> = HashMap<String, AlgorithmParameter<'a>>;

/// Maps parameter names to their owned values.
pub type SerializableParameters = HashMap<String, SerializableParameter>;

/// Maps algorithm names to their parameters.
pub type Config = HashMap<String, SerializableParameters>;

#[derive(Debug)]
pub enum ParameterError {
    TypeMismatch,
}

impl Display for ParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type mismatch")
    }
}

impl std::error::Error for ParameterError {}

/// tries to get a parameters value
///
/// This is only here for backwards compatibility with old algos.
/// It is recommended to just interact with the structs/enums themselves.
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

macro_rules! impl_try_modify_value {
    ($($variant:ident),+ $(,)?) => {
        impl<'a> AlgorithmParameter<'a> {
            /// tries to modify the internally saved value
            ///
            /// This can be used to try and set the parameters of an algorithm to values loaded from a config.
            pub fn try_modify_value(&'a mut self, set_to: SerializableParameter) -> Result<(), ParameterError> {
                match self {
                    $(Self::$variant(v) => {
                        **v = set_to.try_into()?;
                    })+
                }
                Ok(())
            }
        }
    };
}

macro_rules! impl_to_serializable {
    ($($variant:ident),+ $(,)?) => {
        impl<'a> From<AlgorithmParameter<'a>> for SerializableParameter {
            fn from(value: AlgorithmParameter<'a>) -> Self {
                match value {$(
                    AlgorithmParameter::$variant(v) => Self::$variant(*v),
                )+}
            }
        }
    };
}

macro_rules! impl_from_both_ways {
    ($from_type:ty, $enum_variant:ident) => {
        impl From<$from_type> for SerializableParameter {
            fn from(value: $from_type) -> Self {
                Self::$enum_variant(value)
            }
        }
        impl TryFrom<SerializableParameter> for $from_type {
            type Error = ParameterError;
            fn try_from(value: SerializableParameter) -> Result<$from_type, Self::Error> {
                match value {
                    SerializableParameter::$enum_variant(v) => Ok(v),
                    _ => Err(ParameterError::TypeMismatch),
                }
            }
        }
        impl<'a> From<&'a mut $from_type> for AlgorithmParameter<'a> {
            fn from(value: &'a mut $from_type) -> Self {
                Self::$enum_variant(value)
            }
        }
        impl<'a> TryFrom<AlgorithmParameter<'a>> for &'a mut $from_type {
            type Error = ParameterError;
            fn try_from(value: AlgorithmParameter<'a>) -> Result<&'a mut $from_type, Self::Error> {
                match value {
                    AlgorithmParameter::$enum_variant(v) => Ok(v),
                    _ => Err(ParameterError::TypeMismatch),
                }
            }
        }
    };

    ($(($from_type:ty, $enum_variant:ident)),+ $(,)?) => {$(
        impl_from_both_ways!($from_type, $enum_variant);
    )+};
}

macro_rules! giant_maker_macro {
    ($(($variant:ident, $inner_type:ty)),+ $(,)?) => {
        define_enums!($(($variant, $inner_type)),+);
        impl_display!($($variant),+);
        impl_try_modify_value!($($variant),+);
        impl_to_serializable!($($variant),+);
        impl_from_both_ways!($(($inner_type, $variant)),+);
    };
}

giant_maker_macro!(
    (Bool, bool),
    // (Int8, i8),
    // (Int16, i16),
    (Int32, i32),
    (Int64, i64),
    // (Int128, i128),
    (Unsigned8, u8),
    // (Unsigned16, u16),
    (Unsigned32, u32),
    (Unsigned64, u64),
    // (Unsigned128, u128),
    // (Isize, isize),
    (Usize, usize),
    (Float32, f32),
    (Float64, f64),
    // (Char, char),
);

/// helper macro for creating `AlgorithmParameters` object from a struct
///
/// # Example
/// ```
/// # use analysis_template::make_algorithm_params;
/// # use analysis_template::lib::parameters::{AlgorithmParameters, AlgorithmParameter};
/// // This struct could be the algorithm's struct or a special parameters struct
/// struct SomeStruct {
///     a_toggle: bool,
///     some_specific_value: f32,
///     a_maximum_count: u32,
///     
///     some_other_val: u32,
/// }
///
/// let mut some_struct = SomeStruct {
///     a_toggle: true,
///     some_specific_value: 12.34,
///     a_maximum_count: 69,
///     some_other_val: 42,
/// };
/// let algorithm_params: AlgorithmParameters<'_> = make_algorithm_params!(
///     // the variable, off which to take fields
///     // must be mutable
///     some_struct;
///
///     // a list of fields to convert to `AlgorithmParameter`s
///     // they will be inserted into the `AlgorithmParameters` with the name of that field
///     [
///         a_toggle,
///         some_specific_value,
///         a_maximum_count,
///     ]
/// );
/// assert!(matches!(algorithm_params.get("a_toggle"), Some(&AlgorithmParameter::Bool(&mut true))));
/// assert!(matches!(algorithm_params.get("some_specific_value"), Some(&AlgorithmParameter::Float32(&mut 12.34))));
/// assert!(matches!(algorithm_params.get("a_maximum_count"), Some(&AlgorithmParameter::Unsigned32(&mut 69))));
/// assert!(matches!(algorithm_params.get("some_other_val"), None));
/// ```
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
