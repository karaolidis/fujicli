use std::collections::BTreeMap;

use serde::Deserialize;

use crate::ast::SpecKind;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FujiOption {
    pub id: String,
    pub spec: OptionSpec,
    #[serde(default)]
    pub codegen: Codegen,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Codegen {
    pub skip: bool,
    pub flaky: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum OptionSpec {
    Integer {
        name: String,
        category: Option<String>,
        rules: Option<NumericRules<i32>>,
        encoding: NumericEncoding,
        default: Option<i32>,
    },
    Float {
        name: String,
        category: Option<String>,
        rules: Option<NumericRules<f32>>,
        encoding: NumericEncoding,
        default: Option<f32>,
    },
    String {
        name: String,
        category: Option<String>,
        rules: Option<StringRules>,
        encoding: StringEncoding,
        default: Option<String>,
    },
    Enum {
        name: String,
        category: Option<String>,
        rules: EnumRules,
        encoding: EnumEncoding,
        default: Option<String>,
    },
}

impl OptionSpec {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Integer { name, .. }
            | Self::Float { name, .. }
            | Self::String { name, .. }
            | Self::Enum { name, .. } => name,
        }
    }

    #[must_use]
    pub fn category(&self) -> Option<&str> {
        match self {
            Self::Integer { category, .. }
            | Self::Float { category, .. }
            | Self::String { category, .. }
            | Self::Enum { category, .. } => category.as_deref(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> SpecKind {
        match &self {
            Self::Integer { .. } => SpecKind::Integer,
            Self::Float { .. } => SpecKind::Float,
            Self::String { .. } => SpecKind::String,
            Self::Enum { .. } => SpecKind::Enum,
        }
    }

    #[must_use]
    pub const fn prop_code(&self) -> Option<u16> {
        match self {
            Self::Integer { encoding, .. } | Self::Float { encoding, .. } => encoding.prop_code(),
            Self::String { encoding, .. } => encoding.prop_code(),
            Self::Enum { encoding, .. } => encoding.prop_code(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, bound = "T: Deserialize<'de>")]
pub struct NumericRules<T> {
    pub min: Option<T>,
    pub max: Option<T>,
    pub step: Option<T>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringRules {
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumRules {
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumVariant {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum NumericEncoding {
    Raw {
        prop_code: Option<u16>,
    },
    Scale {
        prop_code: Option<u16>,
        spec: ScaleSpec,
    },
    Lookup {
        prop_code: Option<u16>,
        spec: LookupSpec,
    },
}

impl NumericEncoding {
    #[must_use]
    pub const fn prop_code(&self) -> Option<u16> {
        match self {
            Self::Raw { prop_code }
            | Self::Scale { prop_code, .. }
            | Self::Lookup { prop_code, .. } => *prop_code,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum StringEncoding {
    Raw { prop_code: Option<u16> },
}

impl StringEncoding {
    #[must_use]
    pub const fn prop_code(&self) -> Option<u16> {
        match self {
            Self::Raw { prop_code } => *prop_code,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum EnumEncoding {
    Lookup {
        prop_code: Option<u16>,
        spec: LookupSpec,
    },
}

impl EnumEncoding {
    #[must_use]
    pub const fn prop_code(&self) -> Option<u16> {
        match self {
            Self::Lookup { prop_code, .. } => *prop_code,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaleSpec {
    pub scale: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LookupSpec {
    pub values: BTreeMap<String, LookupValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LookupValue {
    Single(i32),
    Multi(Vec<i32>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_option(json: &str) -> FujiOption {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn lookup_value_single_vs_multi() {
        let opt = parse_option(
            r#"{
                "id": "x", "spec": {
                    "name": "X", "kind": "integer", "rules": { "min": 0, "max": 1 },
                    "encoding": { "kind": "lookup", "spec": { "values": {
                        "a": 5,
                        "b": [1, 2, 3]
                    } } }
                }
            }"#,
        );
        let OptionSpec::Integer { encoding, .. } = opt.spec else {
            panic!()
        };
        let NumericEncoding::Lookup { spec, .. } = encoding else {
            panic!()
        };
        assert!(matches!(spec.values["a"], LookupValue::Single(5)));
        match &spec.values["b"] {
            LookupValue::Multi(v) => assert_eq!(v, &[1, 2, 3]),
            LookupValue::Single(_) => panic!("expected Multi for [1,2,3]"),
        }
    }

    #[test]
    fn codegen_block_defaults_to_skip_false() {
        let opt = parse_option(
            r#"{
                "id": "x", "spec": {
                    "name": "X", "kind": "integer", "encoding": { "kind": "raw" }
                }
            }"#,
        );
        assert!(!opt.codegen.skip);
    }

    #[test]
    fn skip_round_trips() {
        let opt = parse_option(
            r#"{
                "id": "x", "spec": {
                    "name": "X", "kind": "integer", "encoding": { "kind": "raw" }
                },
                "codegen": { "skip": true }
            }"#,
        );
        assert!(opt.codegen.skip);
    }

    #[test]
    fn category_is_optional_and_round_trips() {
        let without = parse_option(
            r#"{
                "id": "x", "spec": {
                    "name": "X", "kind": "integer", "encoding": { "kind": "raw" }
                }
            }"#,
        );
        assert_eq!(without.spec.category(), None);

        let with = parse_option(
            r#"{
                "id": "x", "spec": {
                    "name": "X", "category": "Tone", "kind": "integer",
                    "encoding": { "kind": "raw" }
                }
            }"#,
        );
        assert_eq!(with.spec.category(), Some("Tone"));
    }

    #[test]
    fn default_is_optional_per_kind_and_round_trips() {
        let int_without = parse_option(
            r#"{ "id": "i", "spec": { "name": "I", "kind": "integer", "encoding": { "kind": "raw" } } }"#,
        );
        let OptionSpec::Integer { default, .. } = int_without.spec else {
            panic!()
        };
        assert_eq!(default, None);

        let int_with = parse_option(
            r#"{ "id": "i", "spec": { "name": "I", "kind": "integer", "encoding": { "kind": "raw" }, "default": 3 } }"#,
        );
        let OptionSpec::Integer { default, .. } = int_with.spec else {
            panic!()
        };
        assert_eq!(default, Some(3));

        let flt_with = parse_option(
            r#"{ "id": "f", "spec": { "name": "F", "kind": "float", "encoding": { "kind": "scale", "spec": { "scale": 10 } }, "default": 0.5 } }"#,
        );
        let OptionSpec::Float { default, .. } = flt_with.spec else {
            panic!()
        };
        assert_eq!(default, Some(0.5));

        let s_with = parse_option(
            r#"{ "id": "s", "spec": { "name": "S", "kind": "string", "encoding": { "kind": "raw" }, "default": "hello" } }"#,
        );
        let OptionSpec::String { default, .. } = s_with.spec else {
            panic!()
        };
        assert_eq!(default.as_deref(), Some("hello"));

        let e_with = parse_option(
            r#"{ "id": "e", "spec": { "name": "E", "kind": "enum", "rules": { "variants": [{ "id": "a", "name": "A", "aliases": [] }] }, "encoding": { "kind": "lookup", "spec": { "values": { "a": 1 } } }, "default": "a" } }"#,
        );
        let OptionSpec::Enum { default, .. } = e_with.spec else {
            panic!()
        };
        assert_eq!(default.as_deref(), Some("a"));
    }

    #[test]
    fn spec_helpers_report_consistently_across_variants() {
        let int = parse_option(
            r#"{ "id": "i", "spec": { "name": "I", "kind": "integer", "encoding": { "kind": "raw" } } }"#,
        );
        let flt = parse_option(
            r#"{ "id": "f", "spec": { "name": "F", "kind": "float", "encoding": { "kind": "scale", "spec": { "scale": 10 } } } }"#,
        );
        let s = parse_option(
            r#"{ "id": "s", "spec": { "name": "S", "kind": "string", "encoding": { "kind": "raw" } } }"#,
        );
        let e = parse_option(
            r#"{ "id": "e", "spec": { "name": "E", "kind": "enum", "rules": { "variants": [] }, "encoding": { "kind": "lookup", "spec": { "values": {} } } } }"#,
        );
        assert_eq!(int.spec.kind(), crate::ast::SpecKind::Integer);
        assert_eq!(flt.spec.kind(), crate::ast::SpecKind::Float);
        assert_eq!(s.spec.kind(), crate::ast::SpecKind::String);
        assert_eq!(e.spec.kind(), crate::ast::SpecKind::Enum);
        assert_eq!(int.spec.name(), "I");
        assert_eq!(e.spec.name(), "E");
    }
}
