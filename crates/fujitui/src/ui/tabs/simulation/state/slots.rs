use std::{slice::Iter, sync::Arc};

use fujicore::{
    CoreError, SupportedCamera,
    features::simulation::SimulationDescriptors,
    generated::{
        options::{CustomSetting, CustomSettingName},
        simulations::SimulationBase,
    },
};
use log::debug;
use thiserror::Error;

use crate::{
    ui::tabs::Buffer,
    workers::{
        ReqId, ReqIdGen,
        device::{DeviceCommand, DeviceHandle},
    },
};

use super::SimulationState;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub(super) enum SlotEntry {
    Loading,
    Loaded(Buffer<SimulationState>),
    Failed(Arc<CoreError>),
}

impl SlotEntry {
    pub const fn name(&self) -> Option<&CustomSettingName> {
        match self {
            Self::Loaded(buf) => buf.working.canonical.custom_setting_name.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(in crate::ui::tabs::simulation) enum FetchSkipError {
    #[error("no device connected")]
    NoDevice,
    #[error("connected camera has no simulation descriptors")]
    NoDescriptors,
    #[error("slots already fetched or in flight")]
    AlreadyFetched,
}

#[derive(Debug, Clone, Error)]
pub(in crate::ui::tabs::simulation) enum SlotError {
    #[error("event arrived for unknown slot {0}")]
    UnknownSlot(CustomSetting),
    #[error("per-slot event arrived for slot {0} that wasn't awaiting data")]
    UnexpectedData(CustomSetting),
}

#[derive(Debug, Default)]
pub(super) struct Slots {
    pub(super) entries: Vec<(CustomSetting, SlotEntry)>,
    pub(super) descriptors: Option<&'static SimulationDescriptors>,
}

impl Slots {
    pub fn request_fetch(
        &mut self,
        device: Option<&DeviceHandle>,
        camera: Option<&'static SupportedCamera>,
        req_gen: &ReqIdGen,
    ) -> Result<ReqId, FetchSkipError> {
        if !self.entries.is_empty() {
            return Err(FetchSkipError::AlreadyFetched);
        }
        let device = device.ok_or(FetchSkipError::NoDevice)?;
        let descriptors = camera
            .and_then(|c| c.simulation)
            .ok_or(FetchSkipError::NoDescriptors)?;
        let slots = descriptors.slots();
        let req = req_gen.next();
        debug!("{req}: fetching {} slots", slots.len());
        self.descriptors = Some(descriptors);
        self.entries = slots.iter().map(|s| (*s, SlotEntry::Loading)).collect();
        device.send(DeviceCommand::FetchSlots { req, slots });
        Ok(req)
    }

    pub fn handle_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        self.expect_awaiting(slot)?;
        self.replace(slot, SlotEntry::Loaded(Buffer::from(self.state_from(base))))
    }

    pub fn handle_fetch_failed(
        &mut self,
        slot: CustomSetting,
        error: Arc<CoreError>,
    ) -> Result<(), SlotError> {
        self.expect_awaiting(slot)?;
        self.replace(slot, SlotEntry::Failed(error))
    }

    fn expect_awaiting(&self, slot: CustomSetting) -> Result<(), SlotError> {
        match self.get(slot) {
            Some(SlotEntry::Loading) => Ok(()),
            Some(_) => Err(SlotError::UnexpectedData(slot)),
            None => Err(SlotError::UnknownSlot(slot)),
        }
    }

    pub fn invalidate(&mut self) {
        self.entries.clear();
    }

    fn replace(&mut self, slot: CustomSetting, entry: SlotEntry) -> Result<(), SlotError> {
        let existing = self
            .entries
            .iter_mut()
            .find(|(s, _)| *s == slot)
            .ok_or(SlotError::UnknownSlot(slot))?;
        existing.1 = entry;
        Ok(())
    }

    pub(super) fn request_refetch(
        &mut self,
        slot: CustomSetting,
        device: Option<&DeviceHandle>,
        req_gen: &ReqIdGen,
    ) -> Option<ReqId> {
        let device = device?;
        let entry = self.entries.iter_mut().find(|(s, _)| *s == slot)?;
        let req = req_gen.next();
        debug!("{req}: refetching slot {slot}");
        entry.1 = SlotEntry::Loading;
        device.send(DeviceCommand::FetchSlot { req, slot });
        Some(req)
    }

    fn state_from(&self, base: &SimulationBase) -> SimulationState {
        let shadow = self
            .descriptors
            .map_or_else(|| base.clone(), |d| d.new_shadow_from(base));
        SimulationState {
            canonical: base.clone(),
            shadow,
        }
    }

    pub fn get(&self, slot: CustomSetting) -> Option<&SlotEntry> {
        self.entries
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, e)| e)
    }

    pub fn get_mut(&mut self, slot: CustomSetting) -> Option<&mut SlotEntry> {
        self.entries
            .iter_mut()
            .find(|(s, _)| *s == slot)
            .map(|(_, e)| e)
    }
}

impl<'a> IntoIterator for &'a Slots {
    type Item = (CustomSetting, &'a SlotEntry);
    type IntoIter = std::iter::Map<
        Iter<'a, (CustomSetting, SlotEntry)>,
        fn(&'a (CustomSetting, SlotEntry)) -> (CustomSetting, &'a SlotEntry),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(s, e)| (*s, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loading(slots: &[CustomSetting]) -> Slots {
        Slots {
            entries: slots.iter().map(|s| (*s, SlotEntry::Loading)).collect(),
            descriptors: None,
        }
    }

    fn named(name: &str) -> SimulationBase {
        SimulationBase {
            custom_setting_name: Some(name.parse().expect("valid name")),
            ..Default::default()
        }
    }

    fn loaded(sim: SimulationBase) -> SlotEntry {
        SlotEntry::Loaded(Buffer::from(SimulationState {
            canonical: sim.clone(),
            shadow: sim,
        }))
    }

    #[test]
    fn per_slot_data_resolves_loading_entries() {
        let mut slots = loading(&[CustomSetting::C1, CustomSetting::C2]);
        slots
            .handle_fetched(CustomSetting::C1, &SimulationBase::default())
            .unwrap();
        assert!(matches!(
            slots.get(CustomSetting::C1),
            Some(SlotEntry::Loaded(_))
        ));
        assert!(matches!(
            slots.get(CustomSetting::C2),
            Some(SlotEntry::Loading)
        ));
        slots
            .handle_fetched(CustomSetting::C2, &SimulationBase::default())
            .unwrap();
        assert!(matches!(
            slots.get(CustomSetting::C2),
            Some(SlotEntry::Loaded(_))
        ));
    }

    #[test]
    fn request_fetch_skips_when_no_device() {
        let mut slots = Slots::default();
        let req_gen = ReqIdGen::new();
        assert_eq!(
            slots.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::NoDevice)
        );
        assert!(slots.entries.is_empty());
    }

    #[test]
    fn request_fetch_blocks_when_entries_present() {
        let mut slots = loading(&[CustomSetting::C1]);
        let req_gen = ReqIdGen::new();
        assert_eq!(
            slots.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::AlreadyFetched)
        );
    }

    #[test]
    fn invalidate_clears_entries() {
        let mut slots = loading(&[CustomSetting::C1, CustomSetting::C2]);
        slots.invalidate();
        assert!(slots.entries.is_empty());
    }

    #[test]
    fn handle_slot_fetched_rejects_unknown_slot() {
        let mut slots = loading(&[CustomSetting::C1]);
        let err = slots
            .handle_fetched(CustomSetting::C2, &SimulationBase::default())
            .unwrap_err();
        assert!(matches!(err, SlotError::UnknownSlot(CustomSetting::C2)));
    }

    #[test]
    fn handle_slot_fetched_rejects_entry_not_awaiting() {
        let mut slots = loading(&[CustomSetting::C1]);
        slots
            .handle_fetched(CustomSetting::C1, &SimulationBase::default())
            .unwrap();
        let err = slots
            .handle_fetched(CustomSetting::C1, &SimulationBase::default())
            .unwrap_err();
        assert!(matches!(err, SlotError::UnexpectedData(CustomSetting::C1)));
    }

    #[test]
    fn slot_entry_name_returns_some_when_loaded_with_name() {
        let entry = loaded(named("Velvia Warm"));
        assert_eq!(
            entry.name().map(ToString::to_string),
            Some("Velvia Warm".to_owned())
        );
    }

    #[test]
    fn slot_entry_name_returns_none_when_loaded_without_name() {
        let entry = loaded(SimulationBase::default());
        assert!(entry.name().is_none());
    }

    #[test]
    fn slot_entry_name_returns_none_when_loading() {
        let entry = SlotEntry::Loading;
        assert!(entry.name().is_none());
    }

    #[test]
    fn slot_entry_name_returns_none_when_failed() {
        let entry = SlotEntry::Failed(Arc::new(CoreError::NoImagingInterface));
        assert!(entry.name().is_none());
    }
}
