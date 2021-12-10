#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    net::{SocketAddr},
    time::Duration,
};

use async_trait::async_trait;

use log::{debug, info};
use tokio::net::UdpSocket;

use crate::{
    client::error::ClientError,
    common::network::remvsp::{RemVSPFrameFragment, RemVSPFrameHeader},
};

use super::{FrameReceiver, ReceivedFrame};

pub struct RemVSPFrameReceiver {
    socket: UdpSocket,
    server_address: SocketAddr,
    state: RemVSPReceptionState,
}

#[derive(Default)]
struct RemVSPReceptionState {
    last_reconstructed_frame: usize,
    frames_in_reception: HashMap<usize, FrameReconstructionState>,
}

#[derive(Default)]
struct FrameReconstructionState {
    frame_header: Option<RemVSPFrameHeader>,
    received_fragments: HashMap<u16, Vec<u8>>,
}

impl FrameReconstructionState {
    pub fn has_received_fragment(&self, fragment: &RemVSPFrameFragment) -> bool {
        self.received_fragments.contains_key(&fragment.fragment_id)
    }

    pub fn register_fragment(&mut self, fragment: RemVSPFrameFragment) {
        if self.frame_header.is_none() {
            self.frame_header = Some(fragment.frame_header);
        }

        self.received_fragments
            .insert(fragment.fragment_id, fragment.data);
    }

    pub fn is_complete(&self) -> bool {
        if self.frame_header.is_some() {
            let received_fragments = self.received_fragments.len() as u16;
            let frame_fragments = self.frame_header.unwrap().frame_fragments_count;

            return received_fragments == frame_fragments;
        }

        return false;
    }

    pub fn reconstruct(self, buffer: &mut [u8]) -> usize {
        let mut written_bytes = 0;

        let frame_header = self.frame_header.expect("Reconstructing without a frame header");

        let fragment_size = frame_header.fragment_size as usize;

        for (fragment_id, data) in self.received_fragments.into_iter() {
            let current_fragment_data_size = data.len();
            let fragment_id = fragment_id as usize;
            let fragment_offset = (fragment_id * fragment_size) as usize;

            let fragment_buffer = 
                &mut buffer[fragment_offset..fragment_offset + current_fragment_data_size];

            fragment_buffer.copy_from_slice(&data);

            written_bytes += current_fragment_data_size;
        }

        written_bytes
    }
}

impl RemVSPFrameReceiver {
    pub async fn connect(port: i16, server_address: SocketAddr) -> Self {
        let binding_address = format!("127.0.0.1:{}", port);

        let socket = UdpSocket::bind(binding_address).await.unwrap();

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

            let frame_reception_state = frames_in_reception.get_mut(&frame_id);

            if frame_reception_state.is_some() {
                debug!(
                    "Frame has already been partially received, updating the reconstruction state"
                );
                frame_reception_state.unwrap()
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

    fn reconstruct_frame(&mut self, frame_id: usize, output_buffer: &mut [u8]) -> usize {
        let frame_size = self.state
            .frames_in_reception
            .remove(&frame_id)
            .expect("Retrieving a non-existing frame")
            .reconstruct(output_buffer);

        self.state.last_reconstructed_frame = frame_id;

        frame_size
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
                debug!("Frame #{} has been received completely", frame_id);
                if self.is_frame_stale(frame_id) {
                    debug!("Frame is stale, dropping...");
                    self.drop_frame_data(frame_id);
                    continue
                }

                let buffer_size = self.reconstruct_frame(frame_id, encoded_frame_buffer);

                break Ok(ReceivedFrame {
                    buffer_size,
                    capture_timestamp: frame_header.capture_timestamp,
                    reception_delay: 0,
                })
            }
        };

        result
    }
}
