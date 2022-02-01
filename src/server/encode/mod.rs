use bytes::{Bytes, BytesMut};

use crate::common::feedback::FeedbackMessage;

use super::error::ServerError;

pub mod identity;
pub mod ffmpeg;
pub mod pool;

pub trait Encoder {
    fn encode(&mut self, input_buffer: Bytes, output_buffer: BytesMut) -> Result<usize, ServerError>;
    fn handle_feedback(&mut self, message: FeedbackMessage);
}