#[cfg(test)]
mod tests;

mod reconstruct;

use std::{collections::HashMap, fmt::Debug, net::SocketAddr, time::Duration};

use async_trait::async_trait;

use log::{debug, info};
use socket2::{Domain, Socket, Type};
use tokio::{net::UdpSocket, time::Instant};

use crate::{
    client::error::ClientError,
    common::network::remvsp::{RemVSPFrameFragment, RemVSPFrameHeader},
};

use self::reconstruct::FrameReconstructionState;

use super::{FrameReceiver, ReceivedFrame};

pub struct RemVSPFrameReceiver {
    socket: UdpSocket,
    server_address: SocketAddr,
    state: RemVSPReceptionState,
}

#[derive(Default, Debug)]
struct RemVSPReceptionState {
    last_reconstructed_frame: usize,
    frames_in_reception: HashMap<usize, FrameReconstructionState>,
}

impl RemVSPFrameReceiver {
    pub async fn connect(port: i16, server_address: SocketAddr) -> Self {
        let bind_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let bind_address = bind_address.into();

        let raw_socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();
        raw_socket.set_nonblocking(true).unwrap();
        raw_socket.set_recv_buffer_size(256 * 1024 * 1024).unwrap();
        raw_socket.bind(&bind_address).unwrap();

        let udp_socket: std::net::UdpSocket = raw_socket.into();
        let socket = UdpSocket::from_std(udp_socket).unwrap();

        let hello_buffer = [0; 16];
        socket.send_to(&hello_buffer, server_address).await.unwrap();

        Self {
            socket,
            server_address,
            state: RemVSPReceptionState {
                last_reconstructed_frame: 0,
                ..Default::default()
            },
        }
    }

    fn register_frame_fragment(&mut self, fragment: RemVSPFrameFragment) {
        let frame_id = fragment.frame_header.frame_id;

        let frame_reconstruction_state = {
            let frames_in_reception = &mut self.state.frames_in_reception;

            let frame_reconstruction_state = frames_in_reception.get_mut(&frame_id);

            if frame_reconstruction_state.is_some() {
                debug!(
                    "Frame has already been partially received, updating the reconstruction state"
                );
                frame_reconstruction_state.unwrap()
            } else {
                frames_in_reception.insert(frame_id, FrameReconstructionState::default());
                frames_in_reception.get_mut(&frame_id).unwrap()
            }
        };

        if frame_reconstruction_state.has_received_fragment(&fragment) {
            debug!("Duplicate fragment, dropping");
        } else {
            frame_reconstruction_state.register_fragment(fragment);
        }
    }

    fn is_frame_complete(&self, frame_id: usize) -> bool {
        self.state
            .frames_in_reception
            .get(&frame_id)
            .expect("Retrieving a non-existing frame")
            .is_complete()
    }

    fn is_frame_stale(&self, frame_id: usize) -> bool {
        frame_id <= self.state.last_reconstructed_frame
    }

    fn drop_frame_data(&mut self, frame_id: usize) {
        self.state.frames_in_reception.remove(&frame_id);
    }

    fn reconstruct_frame(&mut self, frame_id: usize, output_buffer: &mut [u8]) -> (u128, usize) {
        let frame = self
            .state
            .frames_in_reception
            .remove(&frame_id)
            .expect("Retrieving a non-existing frame");

        let delay = frame.first_reception.elapsed().as_millis();

        let frame_size = frame.reconstruct(output_buffer);

        self.state.last_reconstructed_frame = frame_id;

        (delay, frame_size)
    }
}

#[async_trait]
impl FrameReceiver for RemVSPFrameReceiver {
    async fn receive_encoded_frame(
        &mut self,
        encoded_frame_buffer: &mut [u8],
    ) -> Result<ReceivedFrame, ClientError> {
        let mut bin_fragment_buffer = vec![0 as u8; 1024];

        let result = loop {
            let received_bytes = self.socket.recv(&mut bin_fragment_buffer).await.unwrap();

            let frame_fragment =
                bincode::deserialize::<RemVSPFrameFragment>(&bin_fragment_buffer[..received_bytes])
                    .unwrap();

            debug!(
                "Received frame fragment #{}: {:?}",
                frame_fragment.fragment_id, frame_fragment.frame_header
            );

            let frame_header = frame_fragment.frame_header;
            let frame_id = frame_header.frame_id;
            self.register_frame_fragment(frame_fragment);

            if self.is_frame_complete(frame_id) {
                debug!(
                    "Frame #{} has been received completely. Last received frame: {}",
                    frame_id, self.state.last_reconstructed_frame
                );

                if self.is_frame_stale(frame_id) {
                    info!("Frame is stale, dropping...");
                    self.drop_frame_data(frame_id);
                    continue;
                }

                debug!("Frames reception state: {:#?}", self.state);

                let (reception_delay, buffer_size) = self.reconstruct_frame(frame_id, encoded_frame_buffer);

                break Ok(ReceivedFrame {
                    buffer_size,
                    capture_timestamp: frame_header.capture_timestamp,
                    reception_delay,
                });
            }
        };

        result
    }
}
