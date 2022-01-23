use std::{sync::{Arc, Mutex}, thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

use bytes::BytesMut;
use chrono::Utc;
use log::{debug, info, warn};
use tokio::{sync::{broadcast, mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender}}, task::JoinHandle};

use crate::{common::{feedback::FeedbackMessage, helpers::silo::channel_pull}, server::{
    capture::FrameCapturer, profiling::TransmittedFrameStats,
    utils::encoding::packed_bgra_to_packed_bgr,
}};

pub struct CaptureResult {
    pub capture_timestamp: u128,
    pub capture_time: Instant,

    pub raw_frame_buffer: BytesMut,
    pub frame_stats: TransmittedFrameStats,
}

pub fn launch_capture_thread(
    spin_time: i64,
    mut raw_frame_buffers_receiver: UnboundedReceiver<BytesMut>,
    capture_result_sender: UnboundedSender<CaptureResult>,
    mut frame_capturer: Box<dyn FrameCapturer + Send>,
    mut feedback_receiver: broadcast::Receiver<FeedbackMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_frame_capture_time: i64 = 0;

        loop {
            debug!("Capturing frame...");
            match feedback_receiver.try_recv() {
                Ok(message) => { 
                    frame_capturer.handle_feedback(message);
                },
                Err(_) => { }
            };

            tokio::time::sleep(Duration::from_millis(std::cmp::max(
                0,
                spin_time - last_frame_capture_time,
            ) as u64)).await;

            let (mut raw_frame_buffer, raw_frame_buffer_wait_time) =
                channel_pull(&mut raw_frame_buffers_receiver)
                    .await
                    .expect("Raw frame buffers channel closed, terminating.");

            let capture_start_time = Instant::now();

            let capture_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
            let result = frame_capturer.capture();

            debug!("Frame captured");

            let packed_bgra_frame_buffer = result.unwrap();

            packed_bgra_to_packed_bgr(&packed_bgra_frame_buffer, &mut raw_frame_buffer);

            let mut frame_stats = TransmittedFrameStats::default();

            last_frame_capture_time = capture_start_time.elapsed().as_millis() as i64;
            frame_stats.capture_time = last_frame_capture_time as u128;
            frame_stats.capturer_idle_time = raw_frame_buffer_wait_time;

            let send_result = capture_result_sender
                .send(CaptureResult {
                    capture_timestamp,
                    capture_time: capture_start_time,
                    raw_frame_buffer,
                    frame_stats,
                });

            if let Err(e) = send_result {
                warn!("Capture result send error: {}", e);
                break;
            };
        }
    })
}
