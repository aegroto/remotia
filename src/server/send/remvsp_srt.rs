use std::{cmp, net::{SocketAddr, UdpSocket}, time::{Duration, Instant}};

use async_trait::async_trait;

use bytes::Bytes;
use futures::SinkExt;
use log::{debug, info};
use rand::Rng;
use socket2::{Domain, Socket, Type};
use srt_tokio::SrtSocket;

use crate::common::network::remvsp::{RemVSPFrameFragment, RemVSPFrameHeader};

use super::{FrameSender, remvsp::RemVSPFrameSenderConfiguration};

pub struct RemVSPViaSRTFrameSender {
    socket: SrtSocket,
    chunk_size: usize,

    config: RemVSPFrameSenderConfiguration,

    state: RemVSPViaSRTTransmissionState,
}

impl RemVSPViaSRTFrameSender {
    pub async fn listen(port: u16, chunk_size: usize, config: RemVSPFrameSenderConfiguration) -> Self {
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

        Self {
            socket,
            chunk_size,

            config,

            state: RemVSPViaSRTTransmissionState {
                current_frame_id: 1,
            },
        }
    }

    pub async fn send_fragment(&mut self, frame_fragment: &RemVSPFrameFragment) -> usize {
        let bin_fragment = Bytes::from(bincode::serialize(&frame_fragment).unwrap());
        let bytes_to_transmit = bin_fragment.len();

        self.socket
            .send((Instant::now(), bin_fragment))
            .await
            .unwrap();

        debug!(
            "Sent frame fragment #{}: {:?}",
            frame_fragment.fragment_id, frame_fragment.frame_header
        );

        bytes_to_transmit
    }
}

#[derive(Default)]
struct RemVSPViaSRTTransmissionState {
    pub current_frame_id: usize,
}

#[async_trait]
impl FrameSender for RemVSPViaSRTFrameSender {
    async fn send_frame(&mut self, capture_timestamp: u128, frame_buffer: &[u8]) -> usize {
        let chunks = frame_buffer.chunks(self.chunk_size);

        let frame_header = RemVSPFrameHeader {
            frame_id: self.state.current_frame_id,
            frame_fragments_count: chunks.len() as u16,
            fragment_size: self.chunk_size as u16,
            capture_timestamp,
        };

        let mut transmitted_bytes: usize = 0;

        let mut fragments_to_retransmit: Vec<RemVSPFrameFragment> = Vec::new();

        for (idx, chunk) in chunks.enumerate() {
            let frame_fragment = RemVSPFrameFragment {
                frame_header,
                fragment_id: idx as u16,
                data: chunk.to_vec(),
            };

            transmitted_bytes += self.send_fragment(&frame_fragment).await;

            let mut rng = rand::thread_rng();
            if rng.gen::<f32>() < self.config.retransmission_frequency {
                fragments_to_retransmit.push(frame_fragment);
            }
        }

        debug!(
            "Retransmitting {}/{} fragments...",
            fragments_to_retransmit.len(),
            frame_header.frame_fragments_count
        );

        for frame_fragment in fragments_to_retransmit {
            transmitted_bytes += self.send_fragment(&frame_fragment).await
        }

        self.state.current_frame_id += 1;

        transmitted_bytes
    }
}
