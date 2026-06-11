use crate::{
    features::{
        descriptor::{DescriptorTable, OptionDescriptor},
        simulation::SimulationError,
    },
    generated::{options::OptionCategory, renders::RenderBase},
};

#[derive(Debug, Clone, Copy)]
pub struct RenderDescriptors {
    pub fields: &'static [&'static OptionDescriptor<RenderBase>],
    pub validate: fn(RenderBase) -> Result<RenderBase, SimulationError>,
    pub validate_partial: fn(RenderBase) -> Result<RenderBase, SimulationError>,
    pub validate_partial_against:
        fn(RenderBase, &RenderBase) -> Result<RenderBase, SimulationError>,
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

impl DescriptorTable for RenderDescriptors {
    type Base = RenderBase;

    fn fields(&self) -> &'static [&'static OptionDescriptor<RenderBase>] {
        self.fields
    }

    fn visible_fields(&self, canonical: &RenderBase) -> Vec<&'static OptionDescriptor<RenderBase>> {
        Self::visible_fields(self, canonical)
    }

    fn validate_partial(&self, base: RenderBase) -> Option<RenderBase> {
        (self.validate_partial)(base).ok()
    }

    fn validate_partial_against(
        &self,
        base: RenderBase,
        original: &RenderBase,
    ) -> Option<RenderBase> {
        (self.validate_partial_against)(base, original).ok()
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
    fn dynamic_range_cannot_exceed_the_shot_value() {
        use crate::generated::options::DynamicRange;

        let Some(descriptors) = crate::generated::cameras::SUPPORTED
            .iter()
            .find_map(|cam| cam.render)
        else {
            return;
        };

        let mut shot = descriptors.new_canonical_default();
        shot.dynamic_range = Some(DynamicRange::Hdr100);
        let shot = (descriptors.validate_partial)(shot).expect("shot profile settles");
        assert_eq!(shot.dynamic_range, Some(DynamicRange::Hdr100));

        let mut bumped = shot.clone();
        bumped.dynamic_range = Some(DynamicRange::Hdr400);
        assert_eq!(
            (descriptors.validate_partial)(bumped.clone())
                .expect("partial settles")
                .dynamic_range,
            Some(DynamicRange::Hdr400),
        );

        if let Ok(validated) = (descriptors.validate_partial_against)(bumped, &shot) {
            assert_ne!(validated.dynamic_range, Some(DynamicRange::Hdr400));
        }
    }

    #[test]
    fn dynamic_range_plus_round_trips_through_partial_validation() {
        use crate::generated::options::{DynamicRange, DynamicRangePriority};

        let Some(descriptors) = crate::generated::cameras::SUPPORTED
            .iter()
            .find_map(|cam| cam.render)
        else {
            return;
        };

        let mut base = descriptors.new_canonical_default();
        base.dynamic_range = Some(DynamicRange::Hdr800Plus);
        let settled = (descriptors.validate_partial)(base).expect("partial settles");
        assert_eq!(settled.dynamic_range, Some(DynamicRange::Hdr800Plus));
        assert_eq!(settled.dynamic_range_priority, None);

        let mut decomposed = descriptors.new_canonical_default();
        decomposed.dynamic_range = Some(DynamicRange::Hdr800);
        decomposed.dynamic_range_priority = Some(DynamicRangePriority::Plus);
        let lifted = (descriptors.validate_partial)(decomposed).expect("partial settles");
        assert_eq!(lifted.dynamic_range, Some(DynamicRange::Hdr800Plus));
        assert_eq!(lifted.dynamic_range_priority, None);
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
