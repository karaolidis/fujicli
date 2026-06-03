use serde::Deserialize;
use serde_json::Value;

use crate::ast::{LeafEquals, LeafPresent, Predicate, Scope};

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
    pub generation: String,
    pub usb: Usb,
    pub features: Option<Features>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usb {
    pub vendor_id: u16,
    pub product_id: u16,
    pub chunk_size: u32,
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
    pub transformations: Vec<Transformation>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Render {
    pub profile_code: u32,
    pub fields: Vec<Field>,
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

#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Field {
    Ref(FieldRef),
    Inline(FieldInline),
}

impl Field {
    pub fn id(&self) -> &str {
        match self {
            Field::Ref(r) => &r.id,
            Field::Inline(i) => &i.id,
        }
    }

    pub fn skip_read(&self) -> bool {
        match self {
            Field::Ref(r) => r.skip_read,
            Field::Inline(i) => i.skip_read,
        }
    }

    pub fn skip_write(&self) -> bool {
        match self {
            Field::Ref(r) => r.skip_write,
            Field::Inline(i) => i.skip_write,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRef {
    pub id: String,
    pub r#ref: String,
    #[serde(default)]
    pub skip_read: bool,
    #[serde(default)]
    pub skip_write: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldInline {
    pub id: String,
    #[serde(default)]
    pub skip_read: bool,
    #[serde(default)]
    pub skip_write: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpecKind {
    Integer,
    Float,
    String,
    Enum,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transformation {
    pub when: Option<Predicate>,
    pub apply: Vec<Assignment>,
}

#[derive(Clone, Debug)]
pub struct Assignment {
    pub r#ref: String,
    pub effect: AssignmentEffect,
}

#[derive(Clone, Debug)]
pub enum AssignmentEffect {
    Set(Value),
    Clear,
}

impl From<&Assignment> for Predicate {
    fn from(a: &Assignment) -> Self {
        match &a.effect {
            AssignmentEffect::Set(v) => LeafEquals {
                r#ref: a.r#ref.clone(),
                scope: Scope::Current,
                equals: v.clone(),
            }
            .into(),
            AssignmentEffect::Clear => LeafPresent {
                r#ref: a.r#ref.clone(),
                scope: Scope::Current,
                present: false,
            }
            .into(),
        }
    }
}

impl<'de> Deserialize<'de> for Assignment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            r#ref: String,
            #[serde(default)]
            value: Option<Value>,
            #[serde(default)]
            present: Option<bool>,
        }

        let raw = Raw::deserialize(deserializer)?;
        match (raw.value, raw.present) {
            (Some(value), None) => Ok(Assignment {
                r#ref: raw.r#ref,
                effect: AssignmentEffect::Set(value),
            }),
            (None, Some(false)) => Ok(Assignment {
                r#ref: raw.r#ref,
                effect: AssignmentEffect::Clear,
            }),
            _ => unreachable!("cue should ensure exactly one of the variants is present"),
        }
    }
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
        parse_err::<Setting>(r#"{ "id": "head_0" }"#);
    }

    #[test]
    fn field_disambiguates_ref_vs_inline_by_presence_of_ref() {
        let r: Field = parse(r#"{ "id": "x", "ref": "image_size" }"#);
        let i: Field = parse(r#"{ "id": "head_0" }"#);
        assert!(matches!(r, Field::Ref(_)));
        assert!(matches!(i, Field::Inline(_)));
    }

    #[test]
    fn field_skip_flags_default_false_and_round_trip() {
        let plain: Field = parse(r#"{ "id": "x", "ref": "image_size" }"#);
        let Field::Ref(plain) = plain else { panic!() };
        assert!(!plain.skip_read && !plain.skip_write);

        let both: Field =
            parse(r#"{ "id": "x", "ref": "image_size", "skip_read": true, "skip_write": true }"#);
        let Field::Ref(both) = both else { panic!() };
        assert!(both.skip_read && both.skip_write);

        let inline: Field = parse(r#"{ "id": "x", "skip_read": true }"#);
        let Field::Inline(inline) = inline else {
            panic!()
        };
        assert!(inline.skip_read && !inline.skip_write);
    }

    #[test]
    fn simulation_transformations_and_rules_default_empty() {
        let sim: Simulation = parse(
            r#"{
                "slots": 4,
                "settings": [{ "id": "a", "ref": "image_size" }]
            }"#,
        );
        assert_eq!(sim.slots, 4);
        assert_eq!(sim.settings.len(), 1);
        assert!(sim.transformations.is_empty());
        assert!(sim.rules.is_empty());
    }

    #[test]
    fn transformation_when_is_optional() {
        let t: Transformation = parse(r#"{ "apply": [{ "ref": "x", "value": 0 }] }"#);
        assert!(t.when.is_none());
        assert_eq!(t.apply.len(), 1);
    }

    #[test]
    fn assignment_parses_set_form() {
        let a: Assignment = parse(r#"{ "ref": "x", "value": 5 }"#);
        assert_eq!(a.r#ref, "x");
        assert!(matches!(a.effect, AssignmentEffect::Set(_)));
    }

    #[test]
    fn assignment_parses_clear_form() {
        let a: Assignment = parse(r#"{ "ref": "x", "present": false }"#);
        assert_eq!(a.r#ref, "x");
        assert!(matches!(a.effect, AssignmentEffect::Clear));
    }

    #[test]
    fn features_backup_defaults_false_and_capabilities_optional() {
        let f: Features = parse("{}");
        assert!(!f.backup);
        assert!(f.simulation.is_none() && f.render.is_none());

        let only_render: Features = parse(r#"{ "render": { "profile_code": 1, "fields": [] } }"#);
        assert!(!only_render.backup);
        assert!(only_render.simulation.is_none());
        assert!(only_render.render.is_some());
    }
}
