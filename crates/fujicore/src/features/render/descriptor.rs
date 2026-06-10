use crate::{
    features::{descriptor::OptionDescriptor, simulation::SimulationError},
    generated::{options::OptionCategory, renders::RenderBase},
};

#[derive(Debug, Clone, Copy)]
pub struct RenderDescriptors {
    pub fields: &'static [&'static OptionDescriptor<RenderBase>],
    pub validate: fn(RenderBase) -> Result<RenderBase, SimulationError>,
    pub validate_partial: fn(RenderBase) -> Result<RenderBase, SimulationError>,
}

impl RenderDescriptors {
    pub fn partial_validator(&self) -> impl Fn(RenderBase) -> Option<RenderBase> + '_ {
        move |b| (self.validate_partial)(b).ok()
    }

    #[must_use]
    pub fn visible_fields(
        &self,
        canonical: &RenderBase,
    ) -> Vec<&'static OptionDescriptor<RenderBase>> {
        let mut categories: Vec<Option<OptionCategory>> = Vec::new();
        for field in self.fields {
            if !categories.contains(&field.category) {
                categories.push(field.category);
            }
        }
        let mut out = Vec::with_capacity(self.fields.len());
        for category in categories {
            for field in self.fields {
                if field.category == category && (field.display)(canonical).is_some() {
                    out.push(*field);
                }
            }
        }
        out
    }

    #[must_use]
    pub fn new_canonical_default(&self) -> RenderBase {
        let mut canonical = RenderBase::default();
        for desc in self.fields {
            let mut candidate = canonical.clone();
            desc.set_default(&mut candidate);
            if let Ok(settled) = (self.validate_partial)(candidate) {
                canonical = settled;
            }
        }
        canonical
    }

    #[must_use]
    pub fn new_shadow_default(&self) -> RenderBase {
        let mut shadow = RenderBase::default();
        for desc in self.fields {
            desc.set_default(&mut shadow);
        }
        shadow
    }

    #[must_use]
    pub fn new_shadow_from(&self, base: &RenderBase) -> RenderBase {
        let mut shadow = base.clone();
        for desc in self.fields {
            if (desc.display)(&shadow).is_some() {
                continue;
            }
            let mut probe = shadow.clone();
            desc.set_default(&mut probe);
            let blocked = (self.validate_partial)(probe.clone())
                .map_or(true, |repaired| (desc.display)(&repaired).is_none());
            if blocked {
                (desc.copy_from)(&mut shadow, &probe);
            }
        }
        shadow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::simulations::SimulationBase;

    #[test]
    fn canonical_default_validates_for_every_render_camera() {
        for cam in crate::generated::cameras::SUPPORTED {
            let Some(descriptors) = cam.render else {
                continue;
            };
            let canonical = descriptors.new_canonical_default();
            let result = (descriptors.validate)(canonical);
            assert!(
                result.is_ok(),
                "render canonical default failed to validate for {}: {result:?}",
                cam.name,
            );
        }
    }

    #[test]
    fn canonical_default_holds_partial_rules_for_every_render_camera() {
        for cam in crate::generated::cameras::SUPPORTED {
            let Some(descriptors) = cam.render else {
                continue;
            };
            let canonical = descriptors.new_canonical_default();
            assert!((descriptors.validate_partial)(canonical).is_ok());
        }
    }

    #[test]
    fn codegen_copy_from_and_eq_round_trip_for_every_render_camera() {
        for cam in crate::generated::cameras::SUPPORTED {
            let Some(descriptors) = cam.render else {
                continue;
            };
            let source = descriptors.new_shadow_default();
            for desc in descriptors.fields {
                if (desc.display)(&source).is_none() {
                    continue;
                }
                let mut target = RenderBase::default();
                (desc.copy_from)(&mut target, &source);
                assert!((desc.eq)(&target, &source));
                assert_eq!((desc.display)(&target), (desc.display)(&source));
            }
        }
    }

    #[test]
    fn render_fields_display_nothing_on_an_unseeded_base() {
        for cam in crate::generated::cameras::SUPPORTED {
            let Some(render) = cam.render else { continue };
            for desc in render.fields {
                assert!((desc.display)(&RenderBase::default()).is_none());
            }
        }
        let _ = SimulationBase::default();
    }
}
