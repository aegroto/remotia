#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::client::error::ClientError;

pub mod udp;
pub mod tcp;
pub mod webrtc;

#[async_trait]
pub trait FrameReceiver {
    async fn receive_encoded_frame(&mut self) -> Result<usize, ClientError>;
    fn get_encoded_frame_buffer(&self) -> Arc<Mutex<Vec<u8>>>;
}