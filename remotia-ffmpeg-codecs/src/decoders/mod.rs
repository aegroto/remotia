#![allow(dead_code)]

use remotia::{error::DropReason, common::feedback::FeedbackMessage};

use async_trait::async_trait;

pub mod h264;

mod utils;

#[async_trait]
pub trait Decoder {
    async fn decode(&mut self, input_buffer: &[u8], output_buffer: &mut [u8]) -> Result<usize, DropReason>;
    fn handle_feedback(&mut self, message: FeedbackMessage);
}