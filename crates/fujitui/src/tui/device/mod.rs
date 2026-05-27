use std::{
    ops::ControlFlow,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use fujicore::{Camera, CoreError};
use log::{debug, error, info};
use rusb::{Device, GlobalContext};

const TICK: Duration = Duration::from_millis(100);
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

pub enum DeviceCommand {}

#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub name: &'static str,
    pub usb_id: String,
    pub battery: u32,
}

#[derive(Debug)]
pub enum DeviceEvent {
    Connected(DeviceSnapshot),
    InfoUpdated(DeviceSnapshot),
    Disconnected,
    Error(Box<CoreError>),
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
            Ok(cmd) => match cmd {},
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
        Err(e) if matches!(e, CoreError::Usb(_) | CoreError::NoImagingInterface) => {
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
        usb_id: camera.connected_usb_id(),
        battery: info.battery(),
    })
}
