#![allow(dead_code)]

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use log::debug;
use tokio::sync::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    Mutex,
};

use async_trait::async_trait;

use super::Encoder;
use crate::{common::feedback::FeedbackMessage, server::error::ServerError};

struct EncodingResult {
    encoding_unit: EncodingUnit,
    encoded_size: usize,
    encoded_frame_buffer: BytesMut,
}

struct EncodingUnit {
    id: u8,
    encoder: Box<dyn Encoder + Send>,
}
unsafe impl Send for EncodingUnit {}

pub struct PoolEncoder {
    encoding_units: Vec<EncodingUnit>,
    encoded_frames_sender: UnboundedSender<EncodingResult>,
    encoded_frames_receiver: UnboundedReceiver<EncodingResult>,
}

unsafe impl Send for PoolEncoder {}

impl PoolEncoder {
    pub fn new(mut encoders: Vec<Box<dyn Encoder + Send>>) -> Self {
        let (encoded_frames_sender, encoded_frames_receiver) =
            mpsc::unbounded_channel::<EncodingResult>();

        let mut encoding_units = Vec::new();
        let mut i = 0;

        while encoders.len() > 0 {
            let encoder = encoders.pop().unwrap();
            encoding_units.push(EncodingUnit { id: i, encoder });

            i += 1;
        }

        Self {
            encoding_units,
            encoded_frames_sender,
            encoded_frames_receiver,
        }
    }
}

#[async_trait]
impl Encoder for PoolEncoder {
    async fn encode(
        &mut self,
        input_buffer: Bytes,
        output_buffer: BytesMut,
    ) -> Result<usize, ServerError> {
        let chosen_encoding_unit = self.encoding_units.pop();

        if chosen_encoding_unit.is_none() {
            return Err(ServerError::NoAvailableEncoders);
        }

        let mut chosen_encoding_unit = chosen_encoding_unit.unwrap();

        let encoder_id = chosen_encoding_unit.id;
        debug!("Encoding with encoder #{}...", encoder_id);

        let result_sender = self.encoded_frames_sender.clone();
        let mut encoded_frame_buffer = output_buffer.clone();

        tokio::spawn(async move {
            let frame_write_buffer = encoded_frame_buffer.split_off(1);

            let encoded_size = chosen_encoding_unit
                .encoder
                .encode(input_buffer, frame_write_buffer.clone())
                .await
                .unwrap();

            encoded_frame_buffer.unsplit(frame_write_buffer);

            encoded_frame_buffer[0] = encoder_id;

            let send_result = result_sender.send(EncodingResult {
                encoding_unit: chosen_encoding_unit,
                encoded_size,
                encoded_frame_buffer,
            });

            if send_result.is_err() {
                panic!("Unhandled pool encoder result channel error on send");
            }
        });

        let encoding_result = self.encoded_frames_receiver.recv().await.unwrap();

        self.encoding_units.push(encoding_result.encoding_unit);

        Ok(encoding_result.encoded_size)
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
