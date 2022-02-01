#![allow(dead_code)]

use bytes::{Bytes, BytesMut};
use log::debug;

use crate::{common::feedback::FeedbackMessage, server::error::ServerError};

use super::Encoder;

pub struct IdentityEncoder { }

impl IdentityEncoder {
    pub fn new() -> Self {
        Self { }
    }
}

impl Encoder for IdentityEncoder {
    fn encode(
        &mut self,
        input_buffer: Bytes,
        mut output_buffer: BytesMut,
    ) -> Result<usize, ServerError> {
        let encoded_frame_length = input_buffer.len();
        output_buffer.copy_from_slice(&input_buffer);
        Ok(encoded_frame_length)
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
