#![allow(dead_code)]

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use log::debug;
use tokio::sync::{Mutex, mpsc::{self, UnboundedSender, UnboundedReceiver}};

use super::Encoder;
use crate::{common::feedback::FeedbackMessage, server::error::ServerError};

struct EncodingUnit {
    id: u8,
    encoder: Box<dyn Encoder + Send>,
    encoded_frames_sender: UnboundedSender<BytesMut>
}
unsafe impl Send for EncodingUnit {}

pub struct PoolEncoder {
    encoding_units: Vec<EncodingUnit>,
    encoded_frames_receiver: UnboundedReceiver<BytesMut>
}

unsafe impl Send for PoolEncoder {}

impl PoolEncoder {
    pub fn new(mut encoders: Vec<Box<dyn Encoder + Send>>) -> Self {
        let (encoded_frames_sender, encoded_frames_receiver) = mpsc::unbounded_channel::<BytesMut>();

        let mut encoding_units = Vec::new();
        let mut i = 0;

        while encoders.len() > 0 {
            let encoder = encoders.pop().unwrap();
            encoding_units.push(EncodingUnit {
                id: i,
                encoder,
                encoded_frames_sender: encoded_frames_sender.clone()
            });

            i += 1;
        }

        Self {
            encoding_units,
            encoded_frames_receiver
        }
    }
}

impl Encoder for PoolEncoder {
    fn encode(
        &mut self,
        input_buffer: Bytes,
        output_buffer: BytesMut,
    ) -> Result<usize, ServerError> {

        let chosen_encoding_unit = self.encoding_units.pop();

        if chosen_encoding_unit.is_none() {
            return Err(ServerError::NoAvailableEncoders);
        }

        let chosen_encoding_unit = chosen_encoding_unit.unwrap();

        let encoder_id = chosen_encoding_unit.id;
        debug!("Encoding with encoder #{}...", encoder_id);

        let mut encoded_frame_buffer = output_buffer.clone();

        tokio::spawn(async move {
            let frame_write_buffer = encoded_frame_buffer.split_off(1);

            let mut encoder = chosen_encoding_unit.encoder;
            encoder.encode(input_buffer, frame_write_buffer.clone()).unwrap();

            encoded_frame_buffer.unsplit(frame_write_buffer);

            encoded_frame_buffer[0] = encoder_id;
        });
        
        Ok(0)
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
