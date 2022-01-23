use std::{
    io::Write,
    net::TcpStream,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;

use bytes::Bytes;
use futures::{stream, SinkExt, StreamExt};

use log::{debug, info, warn};
use serde::Serialize;
use srt_tokio::{SrtSocket, SrtSocketBuilder, options::PacketSize};
use tokio::time::timeout;

use crate::{common::network::srt::SRTFrameBody, server::error::ServerError};

use super::FrameSender;

pub struct SRTFrameSender {
    socket: SrtSocket,

    timeout: Duration,
}

impl SRTFrameSender {
    pub async fn new(port: u16, latency: Duration, timeout: Duration) -> Self {
        info!("Listening...");
        // let socket = SrtSocketBuilder::new_listen()
        //     .latency(latency)
        //     .local_port(port)
        //     .connect()
        //     .await
        //     .unwrap();
        // let listen_address = format!(":{}", port);

        let socket = SrtSocket::builder()
            /*.latency(latency)
            .set(|options| {
                options.connect.timeout = timeout;
                options.sender.max_payload_size = PacketSize(0);
                options.session.peer_idle_timeout = Duration::from_secs(2);
            })*/
            .local_port(port)
            .listen()
            .await
            .unwrap();

        info!("Connected");

        Self { socket, timeout }
    }

    async fn send_item(&mut self, binarized_item: Bytes) {
        self.socket
            .send((Instant::now(), binarized_item))
            .await
            .unwrap();
    }

    async fn send_frame_body(
        &mut self,
        capture_timestamp: u128,
        frame_buffer: &[u8],
    ) -> Result<(), ServerError> {
        debug!("Sending frame body...");

        let obj = SRTFrameBody {
            capture_timestamp,
            frame_pixels: frame_buffer[..std::cmp::min(1024 * 64, frame_buffer.len())].to_vec(),
        };

        let binarized_obj = Bytes::from(bincode::serialize(&obj).unwrap());
        self.send_item(binarized_obj).await;

        Ok(())
    }
}

macro_rules! phase {
    ($future: expr) => {
        if let Err(_) = $future.await {
            return 0;
        }
    };
}

#[async_trait]
impl FrameSender for SRTFrameSender {
    async fn send_frame(&mut self, capture_timestamp: u128, frame_buffer: &[u8]) -> usize {
        phase!(self.send_frame_body(capture_timestamp, frame_buffer));
        debug!(
            "Buffer size: {}, Timestamp: {:?}",
            frame_buffer.len(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        frame_buffer.len()
    }
}
