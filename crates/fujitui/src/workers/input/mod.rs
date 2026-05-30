use std::thread::{self, JoinHandle};

use crossbeam_channel::Sender;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use log::error;

pub fn spawn(tx: Sender<KeyEvent>) -> JoinHandle<()> {
    thread::Builder::new()
        .name("fujitui-input".to_owned())
        .spawn(move || run(&tx))
        .expect("spawning input thread")
}

fn run(tx: &Sender<KeyEvent>) {
    loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if tx.send(key).is_err() {
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("input thread read error: {e}");
                break;
            }
        }
    }
}
