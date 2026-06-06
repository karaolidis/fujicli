pub mod usb;

use std::{
    ops::ControlFlow,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use fujicore::{
    Camera, CoreError, UsbId,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
use log::{debug, error, info};
use rusb::{Device, GlobalContext};

use crate::workers::ReqId;

const TICK: Duration = Duration::from_millis(100);
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
#[allow(dead_code)]
pub enum DeviceCommand {
    FetchSlot {
        req: ReqId,
        slot: CustomSetting,
    },
    FetchAllSlots {
        req: ReqId,
    },
    PushSlot {
        req: ReqId,
        slot: CustomSetting,
        base: SimulationBase,
    },
}

#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub name: &'static str,
    pub usb_id: UsbId,
    pub bus_address: String,
    pub battery: u32,
}

#[derive(Debug)]
pub enum DeviceEvent {
    Connected(DeviceSnapshot),
    InfoUpdated(DeviceSnapshot),
    Disconnected,
    Error(Box<CoreError>),
    SlotsEnumerated {
        req: ReqId,
        slots: Vec<CustomSetting>,
    },
    SlotFetched {
        req: ReqId,
        slot: CustomSetting,
        base: SimulationBase,
    },
    SlotFetchFailed {
        req: ReqId,
        slot: CustomSetting,
        error: Box<CoreError>,
    },
    SlotChanged {
        req: ReqId,
        slot: CustomSetting,
    },
    SlotPushFailed {
        req: ReqId,
        slot: CustomSetting,
        error: Box<CoreError>,
    },
}

pub struct DeviceHandle {
    command_tx: Option<Sender<DeviceCommand>>,
    join: Option<JoinHandle<()>>,
}

impl DeviceHandle {
    pub fn spawn(device: Device<GlobalContext>, event_tx: Sender<DeviceEvent>) -> Self {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let join = thread::Builder::new()
            .name("fujitui-device".to_owned())
            .spawn(move || run(&device, &command_rx, &event_tx))
            .expect("spawning device worker");
        Self {
            command_tx: Some(command_tx),
            join: Some(join),
        }
    }

    #[allow(dead_code)]
    pub fn send(&self, cmd: DeviceCommand) {
        if let Some(tx) = self.command_tx.as_ref() {
            let _ = tx.send(cmd);
        }
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        drop(self.command_tx.take());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run(
    device: &Device<GlobalContext>,
    command_rx: &Receiver<DeviceCommand>,
    event_tx: &Sender<DeviceEvent>,
) {
    let mut camera = match Camera::open(device) {
        Ok(c) => c,
        Err(e) => {
            error!("failed to open camera: {e}");
            let _ = event_tx.send(DeviceEvent::Disconnected);
            return;
        }
    };

    info!("opened camera: {}", camera.name());

    let initial = match snapshot(&mut camera) {
        Ok(snap) => snap,
        Err(e) => {
            error!("initial info fetch failed: {e}");
            let _ = event_tx.send(DeviceEvent::Disconnected);
            return;
        }
    };

    if event_tx.send(DeviceEvent::Connected(initial)).is_err() {
        return;
    }

    let mut last_refresh = Instant::now();

    loop {
        match command_rx.recv_timeout(TICK) {
            Ok(cmd) => {
                let outcome = match cmd {
                    DeviceCommand::FetchSlot { req, slot } => {
                        fetch_slot(&mut camera, req, slot, event_tx)
                    }
                    DeviceCommand::FetchAllSlots { req } => {
                        fetch_all_slots(&mut camera, req, event_tx)
                    }
                    DeviceCommand::PushSlot { req, slot, base } => {
                        push_slot(&mut camera, req, slot, base, event_tx)
                    }
                };
                if outcome.is_break() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if last_refresh.elapsed() >= REFRESH_INTERVAL {
                    if refresh(&mut camera, event_tx).is_break() {
                        return;
                    }
                    last_refresh = Instant::now();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                debug!("device worker command channel closed");
                break;
            }
        }
    }
}

fn refresh(camera: &mut Camera, event_tx: &Sender<DeviceEvent>) -> ControlFlow<()> {
    match snapshot(camera) {
        Ok(snap) => {
            let _ = event_tx.send(DeviceEvent::InfoUpdated(snap));
            ControlFlow::Continue(())
        }
        Err(e) if is_disconnect(&e) => {
            error!("camera appears disconnected: {e}");
            let _ = event_tx.send(DeviceEvent::Disconnected);
            ControlFlow::Break(())
        }
        Err(e) => {
            error!("info refresh failed: {e}");
            let _ = event_tx.send(DeviceEvent::Error(Box::new(e)));
            ControlFlow::Continue(())
        }
    }
}

fn snapshot(camera: &mut Camera) -> Result<DeviceSnapshot, CoreError> {
    let info = camera.get_info()?;
    Ok(DeviceSnapshot {
        name: camera.name(),
        usb_id: camera.usb_id(),
        bus_address: camera.bus_address(),
        battery: info.battery(),
    })
}

fn fetch_slot(
    camera: &mut Camera,
    req: ReqId,
    slot: CustomSetting,
    event_tx: &Sender<DeviceEvent>,
) -> ControlFlow<()> {
    match camera.get_simulation(slot) {
        Ok(sim) => {
            let _ = event_tx.send(DeviceEvent::SlotFetched {
                req,
                slot,
                base: sim.to_base(),
            });
            ControlFlow::Continue(())
        }
        Err(e) if is_disconnect(&e) => {
            error!("{req}: camera disconnected during fetch of slot {slot}: {e}");
            let _ = event_tx.send(DeviceEvent::Disconnected);
            ControlFlow::Break(())
        }
        Err(e) => {
            error!("{req}: fetch slot {slot} failed: {e}");
            let _ = event_tx.send(DeviceEvent::SlotFetchFailed {
                req,
                slot,
                error: Box::new(e),
            });
            ControlFlow::Continue(())
        }
    }
}

fn fetch_all_slots(
    camera: &mut Camera,
    req: ReqId,
    event_tx: &Sender<DeviceEvent>,
) -> ControlFlow<()> {
    let slots = match camera.custom_settings_slots() {
        Ok(s) => s,
        Err(e) if is_disconnect(&e) => {
            error!("{req}: camera disconnected enumerating slots: {e}");
            let _ = event_tx.send(DeviceEvent::Disconnected);
            return ControlFlow::Break(());
        }
        Err(e) => {
            error!("{req}: enumerating slots failed: {e}");
            let _ = event_tx.send(DeviceEvent::Error(Box::new(e)));
            return ControlFlow::Continue(());
        }
    };
    let _ = event_tx.send(DeviceEvent::SlotsEnumerated {
        req,
        slots: slots.clone(),
    });
    for slot in slots {
        fetch_slot(camera, req, slot, event_tx)?;
    }
    ControlFlow::Continue(())
}

fn push_slot(
    camera: &mut Camera,
    req: ReqId,
    slot: CustomSetting,
    base: SimulationBase,
    event_tx: &Sender<DeviceEvent>,
) -> ControlFlow<()> {
    match camera.update_simulation(slot, base) {
        Ok(()) => {
            let _ = event_tx.send(DeviceEvent::SlotChanged { req, slot });
            ControlFlow::Continue(())
        }
        Err(e) if is_disconnect(&e) => {
            error!("{req}: camera disconnected during push to slot {slot}: {e}");
            let _ = event_tx.send(DeviceEvent::Disconnected);
            ControlFlow::Break(())
        }
        Err(e) => {
            error!("{req}: push slot {slot} failed: {e}");
            let _ = event_tx.send(DeviceEvent::SlotPushFailed {
                req,
                slot,
                error: Box::new(e),
            });
            ControlFlow::Continue(())
        }
    }
}

const fn is_disconnect(e: &CoreError) -> bool {
    matches!(e, CoreError::Usb(_) | CoreError::NoImagingInterface)
}
