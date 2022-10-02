use anyhow::Result;
use std::sync::mpsc::{Receiver, Sender};

pub trait Device {
    fn sender(&self) -> Result<Sender<Vec<u8>>>;
    fn receiver(&mut self) -> Result<Receiver<Vec<u8>>>;
}
