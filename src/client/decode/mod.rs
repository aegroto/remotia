#![allow(dead_code)]

use crate::{client::error::ClientError, common::feedback::FeedbackMessage};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};

pub mod identity;
pub mod h264;
pub mod h264rgb;
pub mod h265;
pub mod pool;

mod utils;

#[async_trait]
pub trait Decoder {
    async fn decode(&mut self, input_buffer: Bytes, output_buffer: &mut BytesMut) -> Result<usize, ClientError>;
    fn handle_feedback(&mut self, message: FeedbackMessage);
}