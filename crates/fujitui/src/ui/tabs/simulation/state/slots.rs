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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui::tabs::simulation) enum SlotsState {
    /// No fetch has been requested.
    #[default]
    Idle,
    /// Waiting for the device to echo back the slot list.
    Requested(ReqId),
    /// Slot list received; awaiting per-slot data.
    InFlight(ReqId),
    /// All slots have resolved.
    Loaded,
    /// Device dropped; refetch is permitted once a device reappears.
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(in crate::ui::tabs::simulation) enum FetchSkipError {
    #[error("no device connected")]
    NoDevice,
    #[error("connected camera has no simulation descriptors")]
    NoDescriptors,
    #[error("a fetch is already requested")]
    AlreadyRequested,
    #[error("a fetch is already in flight")]
    AlreadyInFlight,
    #[error("slots already loaded")]
    AlreadyLoaded,
}

#[derive(Debug, Clone, Error)]
pub(in crate::ui::tabs::simulation) enum SlotError {
    #[error("slots enumeration arrived in state {state:?} with req {req} we didn't issue")]
    UnexpectedEnumeration { state: SlotsState, req: ReqId },
    #[error("event arrived for unknown slot {0}")]
    UnknownSlot(CustomSetting),
    #[error("per-slot event arrived while no fetch was in flight")]
    NoFetchInFlight,
}

#[derive(Debug, Default)]
pub(super) struct Slots {
    pub(super) state: SlotsState,
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
        match self.state {
            SlotsState::Requested(_) => return Err(FetchSkipError::AlreadyRequested),
            SlotsState::InFlight(_) => return Err(FetchSkipError::AlreadyInFlight),
            SlotsState::Loaded => return Err(FetchSkipError::AlreadyLoaded),
            SlotsState::Idle | SlotsState::Stale => {}
        }
        let device = device.ok_or(FetchSkipError::NoDevice)?;
        let descriptors = camera
            .and_then(|c| c.simulation)
            .ok_or(FetchSkipError::NoDescriptors)?;
        let req = req_gen.next();
        debug!("{req}: fetching all slots");
        device.send(DeviceCommand::FetchAllSlots { req });
        self.state = SlotsState::Requested(req);
        self.descriptors = Some(descriptors);
        Ok(req)
    }

    pub fn handle_enumerated(
        &mut self,
        req: ReqId,
        slots: &[CustomSetting],
    ) -> Result<(), SlotError> {
        match self.state {
            SlotsState::Requested(r) if r == req => {}
            state => return Err(SlotError::UnexpectedEnumeration { state, req }),
        }
        self.entries = slots.iter().map(|s| (*s, SlotEntry::Loading)).collect();
        self.state = SlotsState::InFlight(req);
        Ok(())
    }

    pub fn handle_enumeration_failed(&mut self, req: ReqId) -> Result<(), SlotError> {
        match self.state {
            SlotsState::Requested(r) if r == req => {
                self.state = SlotsState::Idle;
                Ok(())
            }
            state => Err(SlotError::UnexpectedEnumeration { state, req }),
        }
    }

    pub fn handle_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return Err(SlotError::NoFetchInFlight);
        }
        let shadow = self
            .descriptors
            .map_or_else(|| base.clone(), |d| d.new_shadow_from(base));
        let state = SimulationState {
            canonical: base.clone(),
            shadow,
        };
        let buffer = Buffer::from(state);
        self.replace(slot, SlotEntry::Loaded(buffer))?;
        self.advance_if_all_resolved();
        Ok(())
    }

    pub fn handle_fetch_failed(
        &mut self,
        slot: CustomSetting,
        error: Arc<CoreError>,
    ) -> Result<(), SlotError> {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return Err(SlotError::NoFetchInFlight);
        }
        self.replace(slot, SlotEntry::Failed(error))?;
        self.advance_if_all_resolved();
        Ok(())
    }

    pub const fn mark_stale(&mut self) {
        self.state = SlotsState::Stale;
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

    fn advance_if_all_resolved(&mut self) {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return;
        }
        if self
            .entries
            .iter()
            .all(|(_, e)| !matches!(e, SlotEntry::Loading))
        {
            self.state = SlotsState::Loaded;
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

    fn req() -> ReqId {
        ReqIdGen::new().next()
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
    fn handle_slot_fetched_marks_loaded_when_all_resolve() {
        let mut slots = Slots::default();
        let r = req();
        slots.state = SlotsState::Requested(r);
        slots
            .handle_enumerated(r, &[CustomSetting::C1, CustomSetting::C2])
            .unwrap();
        assert!(matches!(slots.state, SlotsState::InFlight(_)));
        slots
            .handle_fetched(CustomSetting::C1, &SimulationBase::default())
            .unwrap();
        assert!(matches!(slots.state, SlotsState::InFlight(_)));
        slots
            .handle_fetched(CustomSetting::C2, &SimulationBase::default())
            .unwrap();
        assert_eq!(slots.state, SlotsState::Loaded);
    }

    #[test]
    fn request_fetch_skips_when_no_device() {
        let mut slots = Slots::default();
        let req_gen = ReqIdGen::new();
        assert_eq!(
            slots.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::NoDevice)
        );
        assert_eq!(slots.state, SlotsState::Idle);
    }

    #[test]
    fn request_fetch_blocks_after_loaded() {
        let mut slots = Slots {
            state: SlotsState::Loaded,
            ..Default::default()
        };
        let req_gen = ReqIdGen::new();
        assert_eq!(
            slots.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::AlreadyLoaded)
        );
    }

    #[test]
    fn mark_stale_unblocks_request() {
        let mut slots = Slots {
            state: SlotsState::Loaded,
            ..Default::default()
        };
        slots.mark_stale();
        assert_eq!(slots.state, SlotsState::Stale);
        let req_gen = ReqIdGen::new();
        assert_eq!(
            slots.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::NoDevice)
        );
    }

    #[test]
    fn handle_slots_enumerated_rejects_mismatched_req() {
        let mut slots = Slots::default();
        let req_gen = ReqIdGen::new();
        let issued = req_gen.next();
        let other = req_gen.next();
        slots.state = SlotsState::Requested(issued);
        let err = slots
            .handle_enumerated(other, &[CustomSetting::C1])
            .unwrap_err();
        assert!(matches!(err, SlotError::UnexpectedEnumeration { .. }));
    }

    #[test]
    fn enumeration_failure_resets_to_idle_and_allows_refetch() {
        let mut slots = Slots::default();
        let issued = req();
        slots.state = SlotsState::Requested(issued);
        slots.handle_enumeration_failed(issued).unwrap();
        assert_eq!(slots.state, SlotsState::Idle);
    }

    #[test]
    fn enumeration_failure_rejects_mismatched_req() {
        let mut slots = Slots::default();
        let req_gen = ReqIdGen::new();
        let issued = req_gen.next();
        let other = req_gen.next();
        slots.state = SlotsState::Requested(issued);
        let err = slots.handle_enumeration_failed(other).unwrap_err();
        assert!(matches!(err, SlotError::UnexpectedEnumeration { .. }));
        assert_eq!(slots.state, SlotsState::Requested(issued));
    }

    #[test]
    fn handle_slot_fetched_rejects_unknown_slot() {
        let mut slots = Slots::default();
        let r = req();
        slots.state = SlotsState::Requested(r);
        slots.handle_enumerated(r, &[CustomSetting::C1]).unwrap();
        let err = slots
            .handle_fetched(CustomSetting::C2, &SimulationBase::default())
            .unwrap_err();
        assert!(matches!(err, SlotError::UnknownSlot(CustomSetting::C2)));
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
