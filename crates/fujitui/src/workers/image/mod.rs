use std::{sync::mpsc, thread};

use log::error;
use ratatui_image::thread::{ResizeRequest, ResizeResponse};

pub struct ImageWorker;

impl ImageWorker {
    pub fn spawn() -> (
        mpsc::Sender<ResizeRequest>,
        crossbeam_channel::Receiver<ResizeResponse>,
    ) {
        let (request_tx, request_rx) = mpsc::channel::<ResizeRequest>();
        let (response_tx, response_rx) = crossbeam_channel::unbounded::<ResizeResponse>();
        thread::Builder::new()
            .name("fujitui-image".to_owned())
            .spawn(move || Self::run(&request_rx, &response_tx))
            .expect("spawning image worker");
        (request_tx, response_rx)
    }

    fn run(
        request_rx: &mpsc::Receiver<ResizeRequest>,
        response_tx: &crossbeam_channel::Sender<ResizeResponse>,
    ) {
        while let Ok(request) = request_rx.recv() {
            match request.resize_encode() {
                Ok(response) => {
                    if response_tx.send(response).is_err() {
                        break;
                    }
                }
                Err(e) => error!("image resize/encode failed: {e}"),
            }
        }
    }
}
