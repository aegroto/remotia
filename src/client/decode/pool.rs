use log::debug;

use crate::client::error::ClientError;

use super::Decoder;

pub struct PoolDecoder {
    decoders: Vec<Box<dyn Decoder>>
}

unsafe impl Send for PoolDecoder { }

impl PoolDecoder {
    pub fn new(decoders: Vec<Box<dyn Decoder>>) -> Self {
        Self {
            decoders
        }
    }
}

impl Decoder for PoolDecoder {
    fn decode(
        &mut self,
        input_buffer: &[u8],
        output_buffer: &mut [u8],
    ) -> Result<usize, ClientError> {
        let chosen_decoder_index = input_buffer[0] as usize;
        let encoded_frame = &input_buffer[1..];

        debug!("Decoding with decoder #{}...", chosen_decoder_index);

        let chosen_decoder = &mut self.decoders[chosen_decoder_index];

        let result = chosen_decoder.decode(encoded_frame, output_buffer);

        result
    }

    fn handle_feedback(&mut self, message: crate::common::feedback::FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}

