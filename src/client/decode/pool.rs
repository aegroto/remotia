use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
use log::debug;
use tokio::sync::{
    mpsc::{self, error::TryRecvError, UnboundedReceiver, UnboundedSender},
    Mutex,
};

use crate::client::error::ClientError;
use async_trait::async_trait;

use super::Decoder;

struct DecodingResult {
    decoding_unit: DecodingUnit,
    decode_result: Result<usize, ClientError>,
}

struct DecodingUnit {
    id: u8,
    decoder: Box<dyn Decoder + Send>,
    output_buffer: BytesMut
}

pub struct PoolDecoder {
    decoding_units: HashMap<u8, DecodingUnit>,
    decoded_frames_sender: UnboundedSender<DecodingResult>,
    decoded_frames_receiver: UnboundedReceiver<DecodingResult>,
}

unsafe impl Send for PoolDecoder {}

impl PoolDecoder {
    pub fn new(buffers_size: usize, mut decoders: Vec<Box<dyn Decoder + Send>>) -> Self {
        let (decoded_frames_sender, decoded_frames_receiver) =
            mpsc::unbounded_channel::<DecodingResult>();

        let mut decoding_units = HashMap::new();
        let mut id = 0;

        while decoders.len() > 0 {
            let decoder = decoders.pop().unwrap();
            decoding_units.insert(
                id,
                DecodingUnit {
                    id,
                    decoder,
                    output_buffer: {
                        let mut buf = BytesMut::with_capacity(buffers_size);
                        buf.resize(buffers_size, 0);
                        buf
                    }
                },
            );

            id += 1;
        }

        Self {
            decoding_units,
            decoded_frames_sender,
            decoded_frames_receiver,
        }
    }

    fn push_to_unit(&mut self, input_buffer: Bytes, mut decoding_unit: DecodingUnit) {
        debug!("Pushing to decoder #{}...", decoding_unit.id);

        let result_sender = self.decoded_frames_sender.clone();

        tokio::spawn(async move {
            let decode_result = decoding_unit
                .decoder
                .decode(input_buffer, &mut decoding_unit.output_buffer)
                .await;

            debug!("Sending decoder #{} result...", decoding_unit.id);

            let send_result = result_sender.send(DecodingResult {
                decoding_unit,
                decode_result,
            });

            if send_result.is_err() {
                panic!("Unhandled pool decoder result channel error on send");
            }
        });
    }
}

#[async_trait]
impl Decoder for PoolDecoder {
    async fn decode(
        &mut self,
        input_buffer: Bytes,
        output_buffer: &mut BytesMut,
    ) -> Result<usize, ClientError> {
        // Push
        let decoding_unit_id = input_buffer[0];

        let decoding_unit = self.decoding_units.remove(&decoding_unit_id);
        let available_decoder = decoding_unit.is_some();

        if available_decoder {
            let input_buffer = input_buffer.slice(1..);
            self.push_to_unit(input_buffer, decoding_unit.unwrap());
        }

        // Pull
        let decoding_result;

        if available_decoder {
            let pull_result = self.decoded_frames_receiver.try_recv();

            if let Err(TryRecvError::Empty) = pull_result {
                debug!("No decoding results");
                return Err(ClientError::NoDecodedFrames);
            }

            decoding_result = pull_result.unwrap();
        } else {
            let pull_result = self.decoded_frames_receiver.recv().await;

            decoding_result = pull_result.unwrap();
        }

        let decoding_unit = decoding_result.decoding_unit;

        output_buffer.copy_from_slice(&decoding_unit.output_buffer);

        self.decoding_units.insert(decoding_unit.id, decoding_unit);

        decoding_result.decode_result
    }

    fn handle_feedback(&mut self, message: crate::common::feedback::FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
