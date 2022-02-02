use std::{ops::ControlFlow, sync::Arc, time::Instant};

use bytes::{Bytes, BytesMut};
use log::{debug, info, warn};
use object_pool::{Pool, Reusable};
use tokio::{
    sync::{
        broadcast::{error::TryRecvError, Receiver},
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
    task::JoinHandle,
};

use crate::{
    common::{feedback::FeedbackMessage, helpers::silo::channel_pull},
    server::{encode::Encoder, error::ServerError, profiling::TransmittedFrameStats},
};

use super::{capture::CaptureResult, types::ServerFrameData};

pub struct EncodeResult {
    pub frame_data: ServerFrameData,
}

pub fn launch_encode_thread(
    mut encoder: Box<dyn Encoder + Send>,
    raw_frame_buffers_sender: UnboundedSender<BytesMut>,
    mut encoded_frame_buffers_receiver: UnboundedReceiver<BytesMut>,
    mut capture_result_receiver: UnboundedReceiver<CaptureResult>,
    encode_result_sender: UnboundedSender<EncodeResult>,
    mut feedback_receiver: Receiver<FeedbackMessage>,
    maximum_capture_delay: u128,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            pull_feedback(&mut feedback_receiver, &mut encoder);

            let (capture_result, capture_result_wait_time) =
                pull_capture_result(&mut capture_result_receiver).await;

            let capture_delay = capture_result.capture_time.elapsed().as_millis();
            let frame_data = capture_result.frame_data;

            let capture_timestamp = frame_data.get("capture_timestamp");

            let raw_frame_buffer = frame_data.extract_writable_buffer("raw_frame_buffer");

            if capture_delay < maximum_capture_delay {
                let (mut encoded_frame_buffer, encoded_frame_buffer_wait_time) =
                    pull_buffer(&mut encoded_frame_buffers_receiver).await;

                let (encode_call_result, encoding_time) = encode(
                    &mut encoder,
                    Bytes::from(raw_frame_buffer.clone()),
                    &mut encoded_frame_buffer,
                )
                .await;

                let (encoded_size, error) = if encode_call_result.is_ok() {
                    let encoded_size = encode_call_result.unwrap();

                    debug!("Encoded size: {}", encoded_size);
                    (encoded_size, None)
                } else {
                    debug!("Error while encoding: {}", encode_call_result.unwrap_err());

                    (0, Some(encode_call_result.unwrap_err()))
                };

                frame_data.set("encoded_size", encoded_size as u128);
                frame_data.set_local("encoding_time", encoding_time);
                frame_data.set_local(
                    "encoder_idle_time",
                    capture_result_wait_time + encoded_frame_buffer_wait_time,
                );
                frame_data.set_local("capture_delay", capture_delay);
                frame_data.set_error(error);

                frame_data.insert_writable_buffer("encoded_frame_buffer", encoded_frame_buffer);

                if let ControlFlow::Break(_) =
                    push_result(&encode_result_sender, EncodeResult { frame_data })
                {
                    break;
                }
            } else {
                debug!("Dropping frame (capture delay: {})", capture_delay);
            }

            return_buffer(&raw_frame_buffers_sender, raw_frame_buffer);
        }
    })
}

fn push_result(
    encode_result_sender: &UnboundedSender<EncodeResult>,
    result: EncodeResult,
) -> ControlFlow<()> {
    let encode_result_sender = encode_result_sender;
    debug!("Pushing encode result...");
    let send_result = encode_result_sender.send(result);
    if let Err(_) = send_result {
        warn!("Transfer result sender error");
        return ControlFlow::Break(());
    };
    ControlFlow::Continue(())
}

fn return_buffer(raw_frame_buffers_sender: &UnboundedSender<BytesMut>, raw_frame_buffer: BytesMut) {
    debug!("Returning empty raw frame buffer...");
    raw_frame_buffers_sender
        .send(raw_frame_buffer)
        .expect("Raw buffer return error");
}

async fn encode(
    encoder: &mut Box<dyn Encoder + Send>,
    raw_frame_buffer: Bytes,
    encoded_frame_buffer: &mut BytesMut,
) -> (Result<usize, ServerError>, u128) {
    let encoding_start_time = Instant::now();
    let encode_call_result = encoder.encode(raw_frame_buffer, encoded_frame_buffer).await;
    let encoding_time = encoding_start_time.elapsed().as_millis();
    (encode_call_result, encoding_time)
}

async fn pull_buffer(
    encoded_frame_buffers_receiver: &mut UnboundedReceiver<BytesMut>,
) -> (BytesMut, u128) {
    debug!("Pulling empty encoded frame buffer...");
    let (encoded_frame_buffer, encoded_frame_buffer_wait_time) =
        channel_pull(encoded_frame_buffers_receiver)
            .await
            .expect("Encoded frame buffers channel closed, terminating.");
    (encoded_frame_buffer, encoded_frame_buffer_wait_time)
}

async fn pull_capture_result(
    capture_result_receiver: &mut UnboundedReceiver<CaptureResult>,
) -> (CaptureResult, u128) {
    debug!("Pulling capture result...");

    let (capture_result, capture_result_wait_time) = channel_pull(capture_result_receiver)
        .await
        .expect("Capture channel closed, terminating.");
    (capture_result, capture_result_wait_time)
}

fn pull_feedback(
    feedback_receiver: &mut Receiver<FeedbackMessage>,
    encoder: &mut Box<dyn Encoder + Send>,
) {
    debug!("Pulling feedback...");
    match feedback_receiver.try_recv() {
        Ok(message) => {
            encoder.handle_feedback(message);
        }
        Err(_) => {}
    };
}
