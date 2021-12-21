use std::{
    cmp,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use async_trait::async_trait;

use log::{debug, info};
use rand::Rng;
use socket2::{Domain, Socket, Type};

use crate::common::network::remvsp::{RemVSPFrameFragment, RemVSPFrameHeader};

use super::FrameSender;

pub struct RemVPSFrameSenderConfiguration {
    retransmission_frequency: f32,
}

impl Default for RemVPSFrameSenderConfiguration {
    fn default() -> Self {
        Self {
            retransmission_frequency: 0.0,
        }
    }
}

pub struct RemVSPFrameSender {
    socket: UdpSocket,
    chunk_size: usize,
    client_address: SocketAddr,

    config: RemVPSFrameSenderConfiguration,

    state: RemVSPTransmissionState,
}

impl RemVSPFrameSender {
    pub fn listen(port: i16, chunk_size: usize) -> Self {
        let bind_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let bind_address = bind_address.into();

        let raw_socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();
        raw_socket.bind(&bind_address).unwrap();
        raw_socket
            .set_send_buffer_size(chunk_size * 1024 * 1024)
            .unwrap();

        let socket: std::net::UdpSocket = raw_socket.into();

        info!(
            "Socket bound to {:?}, waiting for hello message...",
            bind_address
        );

        let mut hello_buffer = [0; 16];
        let (bytes_received, client_address) = socket.recv_from(&mut hello_buffer).unwrap();
        assert_eq!(bytes_received, 16);

        info!("Hello message received correctly. Streaming...");
        socket
            .set_read_timeout(Some(Duration::from_millis(3000)))
            .unwrap();

        socket.connect(client_address).unwrap();

        Self {
            socket,
            chunk_size,
            client_address,

            config: Default::default(),

            state: RemVSPTransmissionState {
                current_frame_id: 1,
            },
        }
    }

    pub fn send_fragment(&mut self, frame_fragment: &RemVSPFrameFragment) {
        let bin_fragment = bincode::serialize(&frame_fragment).unwrap();

        self.socket.send(&bin_fragment).unwrap();

        debug!(
            "Sent frame fragment #{}: {:?}",
            frame_fragment.fragment_id, frame_fragment.frame_header
        );
    }
}

#[derive(Default)]
struct RemVSPTransmissionState {
    pub current_frame_id: usize,
}

#[async_trait]
impl FrameSender for RemVSPFrameSender {
    async fn send_frame(&mut self, capture_timestamp: u128, frame_buffer: &[u8]) {
        let chunks = frame_buffer.chunks(self.chunk_size);

        let frame_header = RemVSPFrameHeader {
            frame_id: self.state.current_frame_id,
            frame_fragments_count: chunks.len() as u16,
            fragment_size: self.chunk_size as u16,
            capture_timestamp,
        };

        let mut fragments_to_retransmit: Vec<RemVSPFrameFragment> = Vec::new();

        for (idx, chunk) in chunks.enumerate() {
            let frame_fragment = RemVSPFrameFragment {
                frame_header,
                fragment_id: idx as u16,
                data: chunk.to_vec(),
            };

            self.send_fragment(&frame_fragment);

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

        fragments_to_retransmit
            .iter()
            .for_each(|frame_fragment| self.send_fragment(&frame_fragment));

        self.state.current_frame_id += 1;
    }
}
