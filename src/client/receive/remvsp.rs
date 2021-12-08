use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use async_trait::async_trait;

use log::{debug, info};

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
        self.received_fragments
            .insert(fragment.fragment_id, fragment.data);
    }

    pub fn is_complete(&self) -> bool {
        if self.frame_header.is_some() {
            return self.received_fragments.len() as u16
                == self.frame_header.unwrap().frame_fragments_count;
        }

        return false;
    }
}

impl RemVSPFrameReceiver {
    pub fn connect(port: i16, server_address: SocketAddr) -> Self {
        let binding_address = format!("127.0.0.1:{}", port);

        let socket = UdpSocket::bind(binding_address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();

        let hello_buffer = [0; 16];
        socket.send_to(&hello_buffer, server_address).unwrap();

        Self {
            socket,
            server_address,
            state: RemVSPReceptionState::default(),
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
            debug!("Registering new fragment");
            frame_reconstruction_state.register_fragment(fragment);
        }
    }

    fn is_frame_complete(&self, frame_id: usize) -> bool {
        self.state.frames_in_reception.get(&frame_id).unwrap().is_complete()
    }
}

#[async_trait]
impl FrameReceiver for RemVSPFrameReceiver {
    async fn receive_encoded_frame(
        &mut self,
        encoded_frame_buffer: &mut [u8],
    ) -> Result<ReceivedFrame, ClientError> {
        let mut bin_fragment_buffer = vec![0 as u8; 1024];

        loop {
            let received_bytes = self.socket.recv(&mut bin_fragment_buffer).unwrap();

            let frame_fragment =
                bincode::deserialize::<RemVSPFrameFragment>(&bin_fragment_buffer[..received_bytes])
                    .unwrap();

            debug!(
                "Received frame fragment #{}: {:?}",
                frame_fragment.fragment_id, frame_fragment.frame_header
            );

            let frame_id = frame_fragment.frame_header.frame_id;
            self.register_frame_fragment(frame_fragment);

            if self.is_frame_complete(frame_id) {
                debug!("Frame #{} completely received, reconstructing...", frame_id);
                break;
            }
        }

        Ok(ReceivedFrame {
            buffer_size: 0,
            capture_timestamp: 0,
            reception_delay: 0,
        })
    }
}
