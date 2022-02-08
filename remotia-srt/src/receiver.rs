use std::{
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use remotia::{
    client::{
        receive::{FrameReceiver, ReceivedFrame},
    },
    common::{feedback::FeedbackMessage, network::FrameBody},
    types::FrameData,
    traits::FrameProcessor,
};

use futures::TryStreamExt;
use log::{debug, info};
use srt_tokio::{SrtSocket};

use remotia::error::DropReason;

pub struct SRTFrameReceiver {
    socket: SrtSocket,
}

impl SRTFrameReceiver {
    pub async fn new(server_address: &str) -> Self {
        info!("Connecting...");
        let socket = SrtSocket::builder()
            .call(server_address, None)
            .await
            .unwrap();

        info!("Connected");

        Self { socket }
    }

    async fn receive_frame_pixels(
        &mut self,
        frame_buffer: &mut [u8],
    ) -> Result<ReceivedFrame, DropReason> {
        debug!("Receiving encoded frame bytes...");

        let receive_result = self.socket.try_next().await;

        if let Err(error) = receive_result {
            debug!("Connection error: {:?}", error);
            return Err(DropReason::ConnectionError);
        }

        let receive_result = receive_result.unwrap();

        if receive_result.is_none() {
            debug!("None receive result");
            return Err(DropReason::EmptyFrame);
        }

        let (instant, binarized_obj) = receive_result.unwrap();

        match bincode::deserialize::<FrameBody>(&binarized_obj) {
            Ok(body) => {
                let frame_buffer = &mut frame_buffer[..body.frame_pixels.len()];
                frame_buffer.copy_from_slice(&body.frame_pixels);

                let reception_delay = instant.elapsed().as_millis();

                debug!(
                    "Received buffer size: {}, Timestamp: {:?}, Reception delay: {}",
                    frame_buffer.len(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                    reception_delay
                );

                Ok(ReceivedFrame {
                    buffer_size: frame_buffer.len(),
                    capture_timestamp: body.capture_timestamp,
                    reception_delay,
                })
            }
            Err(error) => {
                debug!("Invalid packet error: {:?}", error);
                return Err(DropReason::InvalidPacket);
            }
        }
    }
}

#[async_trait]
impl FrameProcessor for SRTFrameReceiver {
    async fn process(&mut self, mut frame_data: FrameData) -> FrameData {
        let frame_buffer = frame_data
            .get_writable_buffer_ref("encoded_frame_buffer")
            .unwrap();

        match self.receive_frame_pixels(frame_buffer).await {
            Ok(received_frame) => {
                frame_data.set_local("encoded_size", received_frame.buffer_size as u128);
                frame_data.set_local("capture_timestamp", received_frame.capture_timestamp);
                frame_data.set_local("reception_delay", received_frame.reception_delay);
            }
            Err(error) => frame_data.set_error(Some(error)),
        };

        frame_data
    }
}

// retro-compatibility with silo pipeline
#[async_trait]
impl FrameReceiver for SRTFrameReceiver {
    async fn receive_encoded_frame(
        &mut self,
        frame_buffer: &mut [u8],
    ) -> Result<ReceivedFrame, DropReason> {
        self.receive_frame_pixels(frame_buffer).await
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
