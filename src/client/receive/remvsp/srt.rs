use std::{
    collections::HashMap,
    fmt::Debug,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;

use bytes::Bytes;
use futures::TryStreamExt;
use log::{debug, info, warn};
use socket2::{Domain, Socket, Type};
use srt_tokio::SrtSocket;
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    client::error::ClientError,
    common::network::remvsp::{RemVSPFrameFragment, RemVSPFrameHeader},
};

use super::{state::RemVSPReceptionState, FrameReceiver, ReceivedFrame};

pub struct RemVSPViaSRTFrameReceiverConfiguration {
    frame_pull_interval: Duration,
}

impl Default for RemVSPViaSRTFrameReceiverConfiguration {
    fn default() -> Self {
        Self {
            frame_pull_interval: Duration::from_millis(10),
        }
    }
}

pub struct RemVSPViaSRTFrameReceiver {
    server_address: SocketAddr,
    config: RemVSPViaSRTFrameReceiverConfiguration,
    state: Arc<Mutex<RemVSPReceptionState>>,
}

impl RemVSPViaSRTFrameReceiver {
    pub async fn connect(port: i16, server_address: SocketAddr) -> Self {
        let mut obj = Self {
            server_address,
            config: Default::default(),
            state: Arc::new(Mutex::new(RemVSPReceptionState {
                last_reconstructed_frame: 0,
                ..Default::default()
            })),
        };

        obj.run_reception_loop();

        obj
    }

    

    fn run_reception_loop(&mut self) {
        let state = self.state.clone();
        let server_address = self.server_address.clone();

        tokio::spawn(async move {
            let mut socket = SrtSocket::builder()
                /*.latency(latency)
                .set(|options| {
                    options.connect.timeout = timeout;
                    options.receiver.buffer_size = ByteCount(receive_buffer_size);
                    options.session.peer_idle_timeout = Duration::from_secs(2);
                })*/
                .call(server_address, None)
                .await
                .unwrap();

            loop {
                let (_, bin_fragment_buffer) = match receive_item(&mut socket).await {
                    Ok(result) => result,
                    Err(err) => {
                        warn!("Unhandled receive error: {:?}", err);
                        continue;
                    }
                };

                let frame_fragment =
                    bincode::deserialize::<RemVSPFrameFragment>(&bin_fragment_buffer).unwrap();

                debug!(
                    "Received frame fragment #{}: {:?}",
                    frame_fragment.fragment_id, frame_fragment.frame_header
                );

                state.lock().await.register_frame_fragment(frame_fragment);
            }
        });
    }
}

#[async_trait]
impl FrameReceiver for RemVSPViaSRTFrameReceiver {
    async fn receive_encoded_frame(
        &mut self,
        encoded_frame_buffer: &mut [u8],
    ) -> Result<ReceivedFrame, ClientError> {
        let result = {
            debug!("Pulling frame...");
            let received_frame = self.state.lock().await.pull_frame(encoded_frame_buffer);

            match received_frame {
                Some(v) => Ok(v),
                None => Err(ClientError::NoCompleteFrames),
            }
        };

        if let Err(ClientError::NoCompleteFrames) = result {
            debug!("Null pulled frame, throttling...");
            tokio::time::sleep(self.config.frame_pull_interval).await;
        }

        result
    }
}

async fn receive_item(
        socket: &mut SrtSocket,
    ) -> Result<(Instant, Bytes), ClientError> {
        match socket.try_next().await {
            Ok(packet) => {
                if let Some((instant, binarized_obj)) = packet {
                    Ok((instant, binarized_obj))
                } else {
                    warn!("None packet");
                    Err(ClientError::InvalidPacket)
                }
            }
            Err(err) => {
                warn!("Unhandled connection error: {:?}", err);
                return Err(ClientError::ConnectionError);
            }
        }
    }