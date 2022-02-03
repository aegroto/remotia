#![allow(dead_code)]

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use log::debug;
use tokio::sync::{
    mpsc::{self, error::TryRecvError, UnboundedReceiver, UnboundedSender},
    Mutex,
};

use async_trait::async_trait;

use super::Encoder;
use crate::{
    common::feedback::FeedbackMessage,
    server::{error::ServerError, types::ServerFrameData},
};

const POOLING_INFO_SIZE: usize = 1;

struct EncodingResult {
    encoding_unit: EncodingUnit,
    frame_data: ServerFrameData,
}

struct EncodingUnit {
    id: u8,
    encoder: Box<dyn Encoder + Send>,
    cached_raw_frame_buffer: BytesMut,
}
unsafe impl Send for EncodingUnit {}

pub struct PoolEncoder {
    encoding_units: Vec<EncodingUnit>,
    encoded_frames_sender: UnboundedSender<EncodingResult>,
    encoded_frames_receiver: UnboundedReceiver<EncodingResult>,
}

unsafe impl Send for PoolEncoder {}

impl PoolEncoder {
    pub fn new(raw_frame_buffer_size: usize, mut encoders: Vec<Box<dyn Encoder + Send>>) -> Self {
        let (encoded_frames_sender, encoded_frames_receiver) =
            mpsc::unbounded_channel::<EncodingResult>();

        let mut encoding_units = Vec::new();
        let mut i = 0;

        while encoders.len() > 0 {
            let encoder = encoders.pop().unwrap();
            encoding_units.push(EncodingUnit {
                id: i,
                encoder,
                cached_raw_frame_buffer: {
                    let mut buf = BytesMut::with_capacity(raw_frame_buffer_size);
                    buf.resize(raw_frame_buffer_size, 0);
                    buf
                },
            });

            i += 1;
        }

        Self {
            encoding_units,
            encoded_frames_sender,
            encoded_frames_receiver,
        }
    }

    fn push_to_unit(
        &mut self,
        frame_data: &mut ServerFrameData,
        mut chosen_encoding_unit: EncodingUnit,
    ) {
        let encoder_id = chosen_encoding_unit.id;
        debug!("Pushing to encoder #{}...", encoder_id);

        let result_sender = self.encoded_frames_sender.clone();

        // Clone the input raw frame buffer
        let raw_frame_buffer = frame_data
            .extract_writable_buffer("raw_frame_buffer")
            .unwrap();
        chosen_encoding_unit
            .cached_raw_frame_buffer
            .copy_from_slice(&raw_frame_buffer);

        // Extract frame buffers and clone the DTO
        // Then add the buffer to the cloned DTO and send it to the encoding thread
        let encoded_frame_buffer = frame_data
            .extract_writable_buffer("encoded_frame_buffer")
            .expect("No encoded frame buffer in frame DTO");

        let mut local_frame_data = frame_data.clone();

        local_frame_data.insert_writable_buffer("encoded_frame_buffer", encoded_frame_buffer);

        // Add the original raw frame buffer back such that the pipeline may make use of it again
        frame_data.insert_writable_buffer("raw_frame_buffer", raw_frame_buffer);

        // Launch the encoding task
        tokio::spawn(async move {
            // Split the encoding unit internal buffer such that is owned by local frame data
            let local_raw_frame_buffer = chosen_encoding_unit.cached_raw_frame_buffer.split();
            local_frame_data.insert_writable_buffer("raw_frame_buffer", local_raw_frame_buffer);

            let mut output_buffer = local_frame_data
                .get_writable_buffer_ref("encoded_frame_buffer")
                .unwrap()
                .split_to(1);

            chosen_encoding_unit
                .encoder
                .encode(&mut local_frame_data)
                .await;

            let encoded_frame_buffer = local_frame_data
                .extract_writable_buffer("encoded_frame_buffer")
                .unwrap();
            output_buffer.unsplit(encoded_frame_buffer);
            output_buffer[0] = chosen_encoding_unit.id;

            local_frame_data.insert_writable_buffer("encoded_frame_buffer", output_buffer);

            // Unsplit the internal buffer with the slice passed to the encoder
            // Doing so it is owned by the encoding unit again
            chosen_encoding_unit.cached_raw_frame_buffer.unsplit(
                local_frame_data
                    .extract_writable_buffer("raw_frame_buffer")
                    .unwrap(),
            );

            debug!("Sending encoder #{} result...", chosen_encoding_unit.id);
            let send_result = result_sender.send(EncodingResult {
                encoding_unit: chosen_encoding_unit,
                frame_data: local_frame_data,
            });

            if send_result.is_err() {
                panic!("Unhandled pool encoder result channel error on send");
            }
        });
    }
}

#[async_trait]
impl Encoder for PoolEncoder {
    async fn encode(&mut self, frame_data: &mut ServerFrameData) {
        // Push
        let chosen_encoding_unit = self.encoding_units.pop();
        let available_encoders = chosen_encoding_unit.is_some();

        if available_encoders {
            let chosen_encoding_unit = chosen_encoding_unit.unwrap();
            self.push_to_unit(frame_data, chosen_encoding_unit);
        }

        // Pull
        let encoding_result;

        if available_encoders {
            let pull_result = self.encoded_frames_receiver.try_recv();

            if let Err(TryRecvError::Empty) = pull_result {
                debug!("No encoding results");
                frame_data.set_error(Some(ServerError::NoEncodedFrames));
                return;
            }

            encoding_result = pull_result.unwrap();
        } else {
            let pull_result = self.encoded_frames_receiver.recv().await;

            encoding_result = pull_result.unwrap();
        }

        let encoding_unit = encoding_result.encoding_unit;
        let mut local_frame_data = encoding_result.frame_data;
        let encoded_frame_buffer = local_frame_data
            .extract_writable_buffer("encoded_frame_buffer")
            .unwrap();

        // Copy the just pulled frame data in the new DTO
        frame_data.set("capture_timestamp", local_frame_data.get("capture_timestamp"));
        frame_data.set_local("capture_time", local_frame_data.get_local("capture_time"));

        self.encoding_units.push(encoding_unit);

        frame_data.insert_writable_buffer("encoded_frame_buffer", encoded_frame_buffer);
        frame_data.set(
            "encoded_size",
            local_frame_data.get("encoded_size") + POOLING_INFO_SIZE as u128,
        );
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
