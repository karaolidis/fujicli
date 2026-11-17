mod cameras;
mod dnf;
mod grammar;
mod options;
mod processors;
mod profile;
mod sensors;

pub use cameras::*;
pub use dnf::*;
pub use grammar::*;
pub use options::*;
pub use processors::*;
pub use profile::*;
pub use sensors::*;

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fml {
    pub cameras: BTreeMap<String, Camera>,
    pub options: BTreeMap<String, FujiOption>,
    pub processors: BTreeMap<String, Processor>,
    pub profile: Profile,
    pub sensors: BTreeMap<String, Sensor>,
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
        let fml = parse(
            r#"{ "cameras": {}, "options": {}, "processors": {}, "profile": { "fields": [] }, "sensors": {} }"#,
        );
        assert!(
            fml.cameras.is_empty()
                && fml.options.is_empty()
                && fml.processors.is_empty()
                && fml.sensors.is_empty()
        );
    }

    #[test]
    fn each_top_level_field_required() {
        let fields = [
            r#""cameras": {}"#,
            r#""options": {}"#,
            r#""processors": {}"#,
            r#""profile": { "fields": [] }"#,
            r#""sensors": {}"#,
        ];
        for missing in 0..fields.len() {
            let present: Vec<_> = (0..fields.len())
                .filter(|&i| i != missing)
                .map(|i| fields[i])
                .collect();
            parse_err(&format!("{{ {} }}", present.join(", ")));
        }
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
                            "sensor": "sensor_a",
                            "processor": "proc_a",
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
                "processors": {
                    "proc_a": { "id": "proc_a", "spec": { "name": "Proc A" } }
                },
                "profile": { "fields": [] },
                "sensors": {
                    "sensor_a": { "id": "sensor_a", "spec": { "name": "Sensor A" } }
                }
            }"#,
        );
        assert_eq!(fml.cameras["demo"].spec.processor, "proc_a");
        assert_eq!(fml.options["image_size"].spec.name(), "Image Size");
        assert_eq!(fml.processors["proc_a"].spec.name, "Proc A");
        assert_eq!(fml.cameras["demo"].spec.sensor, "sensor_a");
        assert_eq!(fml.sensors["sensor_a"].spec.name, "Sensor A");
    }
}
