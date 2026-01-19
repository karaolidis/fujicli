macro_rules! fuji_i16 {
    ($name:ident, $min:expr, $max:expr, $step:expr, $scale:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, ptp_macro::PtpSerialize, ptp_macro::PtpDeserialize,
        )]
        pub struct $name(i16);

        impl $name {
            pub const MIN: i16 = $min;
            pub const MAX: i16 = $max;
            pub const STEP: i16 = $step;

            pub const SCALE: i16 = $scale;

            pub const RAW_MIN: i16 = $min * $scale;
            pub const RAW_MAX: i16 = $max * $scale;
            pub const RAW_STEP: i16 = $step * $scale;

            pub fn try_from_int(value: i16) -> anyhow::Result<Self> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::MIN) % Self::STEP != 0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::STEP);
                }

                let raw = value * Self::SCALE;

                Ok(Self(raw))
            }

            pub const fn to_int(self) -> i16 {
                self.0 / Self::SCALE
            }
        }

        impl std::ops::Deref for $name {
            type Target = i16;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl std::convert::TryFrom<i16> for $name {
            type Error = anyhow::Error;

            fn try_from(value: i16) -> anyhow::Result<Self> {
                if !(Self::RAW_MIN..=Self::RAW_MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::RAW_MIN) % Self::RAW_STEP != 0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::RAW_STEP);
                }

                Ok(Self(value))
            }
        }

        impl std::convert::From<$name> for i16 {
            fn from(value: $name) -> i16 {
                *value.deref()
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                use crate::ptp::input::CleanAlphanumeric;
                use anyhow::Context;

                let input = s
                    .clean()
                    .parse::<i16>()
                    .with_context(|| format!("Invalid numeric value '{s}'"))?;

                Self::try_from_int(input)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_int())
            }
        }
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_i16(self.to_int())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let val = i16::deserialize(deserializer)?;
                Self::try_from_int(val).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! fuji_i16_frac {
    ($name:ident, $min:expr, $max:expr, $step:expr, $scale:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, ptp_macro::PtpSerialize, ptp_macro::PtpDeserialize,
        )]
        pub struct $name(i16);

        impl $name {
            pub const MIN: f32 = $min;
            pub const MAX: f32 = $max;
            pub const STEP: f32 = $step;

            pub const SCALE: f32 = $scale as f32;

            #[allow(clippy::cast_possible_truncation)]
            pub const RAW_MIN: i16 = ($min * $scale as f32) as i16;
            #[allow(clippy::cast_possible_truncation)]
            pub const RAW_MAX: i16 = ($max * $scale as f32) as i16;
            #[allow(clippy::cast_possible_truncation)]
            pub const RAW_STEP: i16 = ($step * $scale as f32) as i16;

            pub fn try_from_float(value: f32) -> anyhow::Result<Self> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::MIN) % Self::STEP != 0.0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::STEP);
                }

                #[allow(clippy::cast_possible_truncation)]
                let raw = (value * Self::SCALE).round() as i16;

                Ok(Self(raw))
            }

            pub fn to_float(self) -> f32 {
                f32::from(self.0) / Self::SCALE
            }
        }

        impl std::ops::Deref for $name {
            type Target = i16;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl std::convert::TryFrom<i16> for $name {
            type Error = anyhow::Error;

            fn try_from(value: i16) -> anyhow::Result<Self> {
                if !(Self::RAW_MIN..=Self::RAW_MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::RAW_MIN) % Self::RAW_STEP != 0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::RAW_STEP);
                }

                Ok(Self(value))
            }
        }

        impl std::convert::From<$name> for i16 {
            fn from(value: $name) -> i16 {
                *value.deref()
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                use crate::ptp::input::CleanAlphanumeric;
                use anyhow::Context;

                let input = s
                    .clean()
                    .parse::<f32>()
                    .with_context(|| format!("Invalid numeric value '{s}'"))?;

                Self::try_from_float(input)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_float())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_f32(self.to_float())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = f32::deserialize(deserializer)?;
                Self::try_from_float(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! fuji_bool {
    ($name:ident, $true_variant:ident, $false_variant:ident) => {
        #[repr(u16)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            num_enum::IntoPrimitive,
            num_enum::TryFromPrimitive,
            ptp_macro::PtpSerialize,
            ptp_macro::PtpDeserialize,
            strum_macros::EnumIter,
        )]
        pub enum $name {
            $true_variant = 0x1,
            $false_variant = 0x2,
        }

        impl $name {
            pub const fn to_bool(self) -> bool {
                matches!(self, Self::$true_variant)
            }

            pub const fn from_bool(b: bool) -> Self {
                if b {
                    Self::$true_variant
                } else {
                    Self::$false_variant
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let s = match self.to_bool() {
                    true => "On",
                    false => "Off",
                };
                write!(f, "{}", s)
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                match s.clean().as_str() {
                    "true" | "on" => return Ok(Self::$true_variant),
                    "false" | "off" => return Ok(Self::$false_variant),
                    _ => {}
                }

                if let Some(best) = Self::closest(s) {
                    anyhow::bail!(
                        "Unknown {} '{}'. Did you mean '{best}'?",
                        stringify!($name),
                        s
                    );
                }

                anyhow::bail!("Unknown {} '{}'", stringify!($name), s);
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_bool(self.to_bool())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let b = bool::deserialize(deserializer)?;
                Ok(Self::from_bool(b))
            }
        }
    };
}

macro_rules! fuji_enum {
    (
        $(#[$enum_meta:meta])*
        $name:ident, {
            $(
                $(#[$variant_meta:meta])*
                $variant_name:ident = $variant_value:expr, $display_string:literal, [$($match_string:literal),* $(,)?]
            ),* $(,)?
        }
    ) => {
        #[repr(u16)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            num_enum::IntoPrimitive,
            num_enum::TryFromPrimitive,
            ptp_macro::PtpSerialize,
            ptp_macro::PtpDeserialize,
            strum_macros::EnumIter,
        )]
        $(#[$enum_meta])*
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant_name = $variant_value,
            )*
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant_name => write!(f, $display_string),)*
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                match s.clean().as_str() {
                    $($($match_string)|* => return Ok(Self::$variant_name),)*
                    _ => {}
                }

                if let Some(best) = Self::closest(s) {
                    anyhow::bail!("Unknown {} '{s}'. Did you mean '{best}'?", stringify!($name));
                }

                anyhow::bail!("Unknown {} '{s}'", stringify!($name));
            }
        }
    };
}

macro_rules! fuji_try_conv_bits {
    ($name:ident, $from:ty, $to:ty) => {
        impl std::convert::TryFrom<$from> for $name {
            type Error = anyhow::Error;

            fn try_from(value: $from) -> anyhow::Result<Self> {
                let primitive = <$to>::try_from(value)?;
                #[allow(clippy::needless_question_mark)]
                Ok($name::try_from(primitive)?)
            }
        }

        impl std::convert::From<$name> for $from {
            fn from(value: $name) -> $from {
                let primitive: $to = value.into();
                <$from>::from(primitive)
            }
        }
    };
}

pub(crate) use fuji_bool;
pub(crate) use fuji_enum;
pub(crate) use fuji_i16;
pub(crate) use fuji_i16_frac;
pub(crate) use fuji_try_conv_bits;
