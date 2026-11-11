use serde::Deserialize;

/// The conversion profile every camera exchanges, in wire order.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub fields: Vec<Field>,
}

impl Profile {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.id() == id)
    }

    pub fn select<'a>(&'a self, ids: &[String]) -> anyhow::Result<Vec<&'a Field>> {
        if let Some(unknown) = ids.iter().find(|id| self.get(id).is_none()) {
            return Err(anyhow::anyhow!("`{unknown}` is not a profile field"));
        }
        Ok(self
            .fields
            .iter()
            .filter(|f| ids.iter().any(|id| id == f.id()))
            .collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Field {
    Ref(FieldRef),
    Inline(FieldInline),
}

impl Field {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Ref(r) => &r.id,
            Self::Inline(i) => &i.id,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRef {
    pub id: String,
    pub r#ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldInline {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        serde_json::from_str(
            r#"{ "fields": [
                { "id": "head" },
                { "id": "size", "ref": "image_size" },
                { "id": "tail" }
            ] }"#,
        )
        .unwrap()
    }

    #[test]
    fn field_disambiguates_ref_vs_inline_by_presence_of_ref() {
        let p = profile();
        assert!(matches!(p.fields[0], Field::Inline(_)));
        assert!(matches!(p.fields[1], Field::Ref(_)));
    }

    #[test]
    fn select_keeps_wire_order() {
        let p = profile();
        let ids: Vec<&str> = p
            .select(&["tail".into(), "head".into()])
            .unwrap()
            .into_iter()
            .map(Field::id)
            .collect();
        assert_eq!(ids, ["head", "tail"]);
    }

    #[test]
    fn select_rejects_unknown_fields() {
        assert!(profile().select(&["nope".into()]).is_err());
    }
}
