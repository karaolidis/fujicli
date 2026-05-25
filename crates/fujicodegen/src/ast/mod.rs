mod cameras;
mod dnf;
mod generations;
mod grammar;
mod options;

pub use cameras::*;
pub use dnf::*;
pub use generations::*;
pub use grammar::*;
pub use options::*;

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fml {
    pub cameras: BTreeMap<String, Camera>,
    pub options: BTreeMap<String, FujiOption>,
    pub generations: BTreeMap<String, Generation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Fml {
        serde_json::from_str(json).unwrap()
    }

    fn parse_err(json: &str) {
        assert!(
            serde_json::from_str::<Fml>(json).is_err(),
            "expected parse error for: {json}"
        );
    }

    #[test]
    fn minimal_empty_fml() {
        let fml = parse(r#"{ "cameras": {}, "options": {}, "generations": {} }"#);
        assert!(fml.cameras.is_empty() && fml.options.is_empty() && fml.generations.is_empty());
    }

    #[test]
    fn each_top_level_field_required() {
        parse_err(r#"{ "options": {}, "generations": {} }"#);
        parse_err(r#"{ "cameras": {}, "generations": {} }"#);
        parse_err(r#"{ "cameras": {}, "options": {} }"#);
    }

    #[test]
    fn parses_full_minimal_camera_with_simulation() {
        let fml = parse(
            r#"{
                "cameras": {
                    "demo": {
                        "id": "demo",
                        "spec": {
                            "name": "Demo",
                            "generation": "gen_a",
                            "usb": { "vendor_id": 1, "product_id": 2, "chunk_size": 1024 },
                            "features": {
                                "simulation": {
                                    "slots": 1,
                                    "settings": [{ "id": "img", "ref": "image_size" }],
                                    "rules": [{
                                        "message": "demo rule",
                                        "when": { "ref": "img", "scope": "current", "present": true }
                                    }]
                                }
                            }
                        }
                    }
                },
                "options": {
                    "image_size": {
                        "id": "image_size",
                        "spec": {
                            "name": "Image Size", "kind": "enum",
                            "rules": { "variants": [
                                { "id": "small", "name": "Small", "aliases": ["s"] }
                            ] },
                            "encoding": { "kind": "lookup", "spec": { "values": { "small": 1 } } }
                        }
                    }
                },
                "generations": {
                    "gen_a": { "id": "gen_a", "spec": { "name": "Gen A" } }
                }
            }"#,
        );
        assert_eq!(fml.cameras["demo"].spec.generation, "gen_a");
        assert_eq!(fml.options["image_size"].spec.name(), "Image Size");
        assert_eq!(fml.generations["gen_a"].spec.name, "Gen A");
    }
}
