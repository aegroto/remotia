#![allow(dead_code)]

use crate::client::error::ClientError;

pub mod udp;
pub mod tcp;
pub mod srt;

pub trait FrameReceiver {
    fn receive_encoded_frame(&mut self, encoded_frame_buffer: & mut[u8]) -> Result<usize, ClientError>;
}