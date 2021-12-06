#![allow(dead_code)]

use async_trait::async_trait;

use crate::client::error::ClientError;

pub mod udp;
pub mod tcp;
pub mod srt;
pub mod srt_manual_fragmentation;

pub struct ReceivedFrame {
    pub buffer_size: usize,
    pub capture_timestamp: u128,
    pub reception_delay: u128
}

#[async_trait]
pub trait FrameReceiver {
    async fn receive_encoded_frame(&mut self, encoded_frame_buffer: & mut[u8]) -> Result<ReceivedFrame, ClientError>;
}