use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Instant;

use bytes::BytesMut;
use log::{debug, warn};
use object_pool::{Pool, Reusable};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use crate::common::feedback::FeedbackMessage;
use crate::common::helpers::silo::channel_pull;
use crate::server::profiling::TransmittedFrameStats;
use crate::server::send::FrameSender;

use super::encode::EncodeResult;
use super::types::ServerFrameData;

pub struct TransferResult {
    pub frame_data: ServerFrameData,
}

pub fn launch_transfer_thread(
    mut frame_sender: Box<dyn FrameSender + Send>,
    encoded_frame_buffers_sender: UnboundedSender<BytesMut>,
    mut encode_result_receiver: UnboundedReceiver<EncodeResult>,
    transfer_result_sender: UnboundedSender<TransferResult>,
    mut feedback_receiver: broadcast::Receiver<FeedbackMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            pull_feedback(&mut feedback_receiver, &mut frame_sender);

            let (encode_result, encode_result_wait_time) =
                pull_encode_result(&mut encode_result_receiver).await;

            let mut frame_data = encode_result.frame_data;

            let encoded_frame_buffer = frame_data.extract_writable_buffer("encoded_frame_buffer");
            let encoded_size = frame_data.get("encoded_size") as usize;

            if frame_data.get_error().is_none() {
                let capture_timestamp = frame_data.get("capture_timestamp");

                let (transfer_start_time, transmitted_bytes) = transfer(
                    &mut frame_sender,
                    capture_timestamp,
                    &encoded_frame_buffer,
                    encoded_size,
                )
                .await;

                frame_data.set_local("transfer_time", transfer_start_time.elapsed().as_millis());
                frame_data.set_local("transferrer_idle_time", encode_result_wait_time);
                frame_data.set_local("transmitted_bytes", transmitted_bytes as u128);
            } else {
                debug!("Error on encoded frame: {:?}", frame_data.get_error());
            }

            return_buffer(&encoded_frame_buffers_sender, encoded_frame_buffer);

            let send_result = transfer_result_sender.send(TransferResult { frame_data });
            if let Err(_) = send_result {
                warn!("Transfer result sender error");
                break;
            };
        }
    })
}

fn return_buffer(
    encoded_frame_buffers_sender: &UnboundedSender<BytesMut>,
    encoded_frame_buffer: BytesMut,
) {
    debug!("Returning empty encoded frame buffer...");
    encoded_frame_buffers_sender
        .send(encoded_frame_buffer)
        .expect("Encoded frame buffer return error");
}

async fn transfer(
    frame_sender: &mut Box<dyn FrameSender + Send>,
    capture_timestamp: u128,
    encoded_frame_buffer: &BytesMut,
    encoded_size: usize,
) -> (Instant, usize) {
    debug!("Transmitting...");
    let transfer_start_time = Instant::now();
    let transmitted_bytes = frame_sender
        .send_frame(capture_timestamp, &encoded_frame_buffer[..encoded_size])
        .await;

    (transfer_start_time, transmitted_bytes)
}

async fn pull_encode_result(
    encode_result_receiver: &mut UnboundedReceiver<EncodeResult>,
) -> (EncodeResult, u128) {
    debug!("Pulling encode result...");
    let (encode_result, encode_result_wait_time) = channel_pull(encode_result_receiver)
        .await
        .expect("Encode result channel closed, terminating.");
    (encode_result, encode_result_wait_time)
}

fn pull_feedback(
    feedback_receiver: &mut broadcast::Receiver<FeedbackMessage>,
    frame_sender: &mut Box<dyn FrameSender + Send>,
) {
    debug!("Pulling feedback...");
    match feedback_receiver.try_recv() {
        Ok(message) => {
            frame_sender.handle_feedback(message);
        }
        Err(_) => {}
    };
}
