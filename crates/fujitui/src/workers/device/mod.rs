pub mod usb;

use std::{
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use fujicore::{
    Camera, Capability, CoreError, UsbId,
    generated::{options::CustomSetting, renders::RenderBase, simulations::SimulationBase},
};
use log::{debug, error, info};
use rusb::{Device, GlobalContext};

use crate::workers::{ReqId, WorkerHandle};

const TICK: Duration = Duration::from_millis(100);
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum DeviceCommand {
    FetchSlot {
        req: ReqId,
        slot: CustomSetting,
    },
    FetchSlots {
        req: ReqId,
        slots: Vec<CustomSetting>,
    },
    PushSlot {
        req: ReqId,
        slot: CustomSetting,
        base: SimulationBase,
    },
    #[allow(dead_code)]
    Render {
        req: ReqId,
        image: Arc<[u8]>,
        base: RenderBase,
        draft: bool,
    },
    ExportBackup {
        req: ReqId,
    },
    ImportBackup {
        req: ReqId,
        blob: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub name: &'static str,
    pub usb_id: UsbId,
    pub bus_address: String,
    pub battery: u32,
    pub capabilities: &'static [Capability],
}

#[derive(Debug)]
pub enum DeviceEvent {
    Connected(DeviceSnapshot),
    InfoUpdated(DeviceSnapshot),
    Disconnected,
    Error(Box<CoreError>),
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
    RenderStarted {
        req: ReqId,
    },
    RenderDone {
        req: ReqId,
        jpeg: Vec<u8>,
    },
    RenderFailed {
        req: ReqId,
        error: Box<CoreError>,
    },
    BackupExported {
        req: ReqId,
        blob: Vec<u8>,
    },
    BackupExportFailed {
        req: ReqId,
        error: Box<CoreError>,
    },
    BackupImported {
        req: ReqId,
    },
    BackupImportFailed {
        req: ReqId,
        error: Box<CoreError>,
    },
}

pub type DeviceHandle = WorkerHandle<DeviceCommand>;

impl WorkerHandle<DeviceCommand> {
    pub fn spawn(device: Device<GlobalContext>, event_tx: Sender<DeviceEvent>) -> Self {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let inflight = Arc::new(AtomicUsize::new(0));
        let worker_inflight = Arc::clone(&inflight);
        let join = thread::Builder::new()
            .name("fujitui-device".to_owned())
            .spawn(move || DeviceWorker::run(&device, &command_rx, &event_tx, &worker_inflight))
            .expect("spawning device worker");
        Self::new(command_tx, inflight, join)
    }
}

struct DeviceWorker {
    camera: Camera,
}

impl DeviceWorker {
    fn run(
        device: &Device<GlobalContext>,
        command_rx: &Receiver<DeviceCommand>,
        event_tx: &Sender<DeviceEvent>,
        inflight: &AtomicUsize,
    ) {
        let camera = match Camera::open(device) {
            Ok(c) => c,
            Err(e) => {
                error!("failed to open camera: {e}");
                let _ = event_tx.send(DeviceEvent::Disconnected);
                return;
            }
        };

        info!("opened camera: {}", camera.name());

        let mut worker = Self { camera };

        let initial = match worker.snapshot() {
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

        worker.event_loop(command_rx, event_tx, inflight);
    }

    fn event_loop(
        &mut self,
        command_rx: &Receiver<DeviceCommand>,
        event_tx: &Sender<DeviceEvent>,
        inflight: &AtomicUsize,
    ) {
        let mut last_refresh = Instant::now();
        loop {
            match command_rx.recv_timeout(TICK) {
                Ok(cmd) => {
                    let outcome = match cmd {
                        DeviceCommand::FetchSlot { req, slot } => {
                            self.fetch_simulation_slot(req, slot, event_tx)
                        }
                        DeviceCommand::FetchSlots { req, slots } => {
                            self.fetch_simulation_slots(req, &slots, event_tx)
                        }
                        DeviceCommand::PushSlot { req, slot, base } => {
                            self.push_simulation_slot(req, slot, base, event_tx)
                        }
                        DeviceCommand::Render {
                            req,
                            image,
                            base,
                            draft,
                        } => self.render_image(req, &image, &base, draft, event_tx),
                        DeviceCommand::ExportBackup { req } => self.export_backup(req, event_tx),
                        DeviceCommand::ImportBackup { req, blob } => {
                            self.import_backup(req, &blob, event_tx)
                        }
                    };
                    inflight.fetch_sub(1, Ordering::Relaxed);
                    if outcome.is_break() {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if last_refresh.elapsed() >= REFRESH_INTERVAL {
                        if self.refresh(event_tx).is_break() {
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

    fn refresh(&mut self, event_tx: &Sender<DeviceEvent>) -> ControlFlow<()> {
        match self.snapshot() {
            Ok(snap) => {
                let _ = event_tx.send(DeviceEvent::InfoUpdated(snap));
                ControlFlow::Continue(())
            }
            Err(e) if e.is_disconnect() => {
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

    fn snapshot(&mut self) -> Result<DeviceSnapshot, CoreError> {
        let info = self.camera.get_info()?;
        Ok(DeviceSnapshot {
            name: self.camera.name(),
            usb_id: self.camera.usb_id(),
            bus_address: self.camera.bus_address(),
            battery: info.battery(),
            capabilities: self.camera.capabilities(),
        })
    }

    fn fetch_simulation_slot(
        &mut self,
        req: ReqId,
        slot: CustomSetting,
        event_tx: &Sender<DeviceEvent>,
    ) -> ControlFlow<()> {
        match self.camera.get_simulation(slot) {
            Ok(sim) => {
                let _ = event_tx.send(DeviceEvent::SlotFetched {
                    req,
                    slot,
                    base: sim.to_base(),
                });
                ControlFlow::Continue(())
            }
            Err(e) if e.is_disconnect() => {
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

    fn fetch_simulation_slots(
        &mut self,
        req: ReqId,
        slots: &[CustomSetting],
        event_tx: &Sender<DeviceEvent>,
    ) -> ControlFlow<()> {
        for slot in slots {
            self.fetch_simulation_slot(req, *slot, event_tx)?;
        }
        ControlFlow::Continue(())
    }

    fn push_simulation_slot(
        &mut self,
        req: ReqId,
        slot: CustomSetting,
        base: SimulationBase,
        event_tx: &Sender<DeviceEvent>,
    ) -> ControlFlow<()> {
        match self.camera.update_simulation(slot, base) {
            Ok(()) => {
                let _ = event_tx.send(DeviceEvent::SlotChanged { req, slot });
                ControlFlow::Continue(())
            }
            Err(e) if e.is_disconnect() => {
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

    fn render_image(
        &mut self,
        req: ReqId,
        image: &[u8],
        base: &RenderBase,
        draft: bool,
        event_tx: &Sender<DeviceEvent>,
    ) -> ControlFlow<()> {
        let _ = event_tx.send(DeviceEvent::RenderStarted { req });
        match self.camera.render(image, base, draft) {
            Ok(jpeg) => {
                info!("{req}: rendered image ({} bytes)", jpeg.len());
                let _ = event_tx.send(DeviceEvent::RenderDone { req, jpeg });
                ControlFlow::Continue(())
            }
            Err(e) if e.is_disconnect() => {
                error!("{req}: camera disconnected during render: {e}");
                let _ = event_tx.send(DeviceEvent::Disconnected);
                ControlFlow::Break(())
            }
            Err(e) => {
                error!("{req}: render failed: {e}");
                let _ = event_tx.send(DeviceEvent::RenderFailed {
                    req,
                    error: Box::new(e),
                });
                ControlFlow::Continue(())
            }
        }
    }

    fn export_backup(&mut self, req: ReqId, event_tx: &Sender<DeviceEvent>) -> ControlFlow<()> {
        match self.camera.export_backup() {
            Ok(blob) => {
                info!("{req}: exported backup ({} bytes)", blob.len());
                let _ = event_tx.send(DeviceEvent::BackupExported { req, blob });
                ControlFlow::Continue(())
            }
            Err(e) if e.is_disconnect() => {
                error!("{req}: camera disconnected during backup export: {e}");
                let _ = event_tx.send(DeviceEvent::Disconnected);
                ControlFlow::Break(())
            }
            Err(e) => {
                error!("{req}: backup export failed: {e}");
                let _ = event_tx.send(DeviceEvent::BackupExportFailed {
                    req,
                    error: Box::new(e),
                });
                ControlFlow::Continue(())
            }
        }
    }

    fn import_backup(
        &mut self,
        req: ReqId,
        blob: &[u8],
        event_tx: &Sender<DeviceEvent>,
    ) -> ControlFlow<()> {
        match self.camera.import_backup(blob) {
            Ok(()) => {
                info!("{req}: imported backup ({} bytes)", blob.len());
                let _ = event_tx.send(DeviceEvent::BackupImported { req });
                ControlFlow::Continue(())
            }
            Err(e) if e.is_disconnect() => {
                error!("{req}: camera disconnected during backup import: {e}");
                let _ = event_tx.send(DeviceEvent::Disconnected);
                ControlFlow::Break(())
            }
            Err(e) => {
                error!("{req}: backup import failed: {e}");
                let _ = event_tx.send(DeviceEvent::BackupImportFailed {
                    req,
                    error: Box::new(e),
                });
                ControlFlow::Continue(())
            }
        }
    }
}
