use std::num::NonZeroU32;

use serde::{Deserialize, de::Error};

use crate::ast::Predicate;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub id: String,
    pub spec: CameraSpec,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraSpec {
    pub name: String,
    pub sensor: String,
    pub processor: String,
    pub usb: Usb,
    pub features: Option<Features>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usb {
    pub vendor_id: u16,
    pub product_id: u16,
    pub chunk_size: NonZeroU32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Features {
    #[serde(default)]
    pub backup: bool,
    pub simulation: Option<Simulation>,
    pub render: Option<Render>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Simulation {
    pub slots: u32,
    pub settings: Vec<Setting>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Render {
    pub fields: Vec<String>,
    #[serde(default)]
    pub requests: Vec<Request>,
    #[serde(default)]
    pub transformations: Vec<Transformation>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub id: String,
    pub r#ref: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpecKind {
    Integer,
    Float,
    String,
    Enum,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub r#ref: String,
    pub value: String,
    pub then: RequestThen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestThen {
    Keep,
    Becomes(String),
}

impl<'de> Deserialize<'de> for Request {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            r#ref: String,
            value: String,
            #[serde(default)]
            keep: bool,
            #[serde(default)]
            becomes: Option<String>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let then = match (raw.keep, raw.becomes) {
            (true, None) => RequestThen::Keep,
            (false, Some(value)) => RequestThen::Becomes(value),
            _ => {
                return Err(D::Error::custom(
                    "a request needs exactly one of keep and becomes",
                ));
            }
        };
        Ok(Self {
            r#ref: raw.r#ref,
            value: raw.value,
            then,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transformation {
    pub when: Option<Predicate>,
    pub apply: Vec<RawWrite>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawWrite {
    pub r#ref: String,
    pub value: RawValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RawValue {
    Int(i64),
    Variant(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    #[serde(default)]
    pub severity: Severity,
    pub message: String,
    pub when: Predicate,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).unwrap()
    }

    fn parse_err<T: serde::de::DeserializeOwned>(json: &str) {
        assert!(
            serde_json::from_str::<T>(json).is_err(),
            "expected parse error for: {json}"
        );
    }

    #[test]
    fn rule_severity_defaults_to_error_and_round_trips() {
        let default_: Rule = parse(r#"{ "message": "x", "when": true }"#);
        let warn: Rule = parse(r#"{ "severity": "warning", "message": "x", "when": true }"#);
        let info: Rule = parse(r#"{ "severity": "info", "message": "x", "when": true }"#);
        let err: Rule = parse(r#"{ "severity": "error", "message": "x", "when": true }"#);
        assert_eq!(default_.severity, Severity::Error);
        assert_eq!(warn.severity, Severity::Warning);
        assert_eq!(info.severity, Severity::Info);
        assert_eq!(err.severity, Severity::Error);

        parse_err::<Rule>(r#"{ "severity": "critical", "message": "x", "when": true }"#);
    }

    #[test]
    fn setting_requires_ref() {
        let s: Setting = parse(r#"{ "id": "x", "ref": "image_size" }"#);
        assert_eq!(s.id, "x");
        assert_eq!(s.r#ref, "image_size");
        parse_err::<Setting>(r#"{ "id": "shoot_condition" }"#);
    }

    #[test]
    fn simulation_rules_default_empty() {
        let sim: Simulation = parse(
            r#"{
                "slots": 4,
                "settings": [{ "id": "a", "ref": "image_size" }]
            }"#,
        );
        assert_eq!(sim.slots, 4);
        assert_eq!(sim.settings.len(), 1);
        assert!(sim.rules.is_empty());
    }

    #[test]
    fn transformation_when_is_optional() {
        let t: Transformation = parse(r#"{ "apply": [{ "ref": "x", "value": 0 }] }"#);
        assert!(t.when.is_none());
        assert_eq!(t.apply.len(), 1);
    }

    #[test]
    fn raw_write_takes_an_integer_or_a_variant() {
        let int: RawWrite = parse(r#"{ "ref": "x", "value": 3 }"#);
        let variant: RawWrite = parse(r#"{ "ref": "x", "value": "hdr800" }"#);
        assert_eq!(int.value, RawValue::Int(3));
        assert_eq!(variant.value, RawValue::Variant("hdr800".into()));
    }

    #[test]
    fn request_keeps_or_becomes() {
        let keep: Request = parse(r#"{ "ref": "x", "value": "auto", "keep": true }"#);
        let becomes: Request = parse(r#"{ "ref": "x", "value": "fine_raw", "becomes": "fine" }"#);
        assert_eq!(keep.then, RequestThen::Keep);
        assert_eq!(becomes.then, RequestThen::Becomes("fine".into()));
    }

    #[test]
    fn features_backup_defaults_false_and_capabilities_optional() {
        let f: Features = parse("{}");
        assert!(!f.backup);
        assert!(f.simulation.is_none() && f.render.is_none());

        let only_render: Features = parse(r#"{ "render": { "fields": [] } }"#);
        assert!(!only_render.backup);
        assert!(only_render.simulation.is_none());
        assert!(only_render.render.is_some());
    }
}
