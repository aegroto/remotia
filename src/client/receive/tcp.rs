use std::{io::Read, net::{TcpStream}, sync::{Arc, Mutex}};

use async_trait::async_trait;

use log::debug;

use crate::client::error::ClientError;

use super::FrameReceiver;

pub struct TCPFrameReceiver {
    stream: TcpStream,
    encoded_frame_buffer: Arc<Mutex<Vec<u8>>>
}

impl TCPFrameReceiver {
    pub fn create(
        stream: TcpStream,
        encoded_frame_buffer_size: usize
    ) -> Self {
        Self {
            stream,
            encoded_frame_buffer: Arc::new(Mutex::new(vec![0 as u8; encoded_frame_buffer_size]))
        }
    }

    fn receive_frame_header(&mut self) -> Result<usize, ClientError> {
        debug!("Receiving frame header...");

        let mut frame_size_vec = [0 as u8; 8];

        let result = self.stream.read(&mut frame_size_vec);

        if result.is_err() {
            return Err(ClientError::InvalidWholeFrameHeader);
        }

        Ok(usize::from_be_bytes(frame_size_vec))
    }

    fn receive_frame_pixels(&mut self)  -> Result<usize, ClientError> {
        let mut unlocked_encoded_frame_buffer = self.encoded_frame_buffer.lock().unwrap();

        debug!("Receiving {} encoded frame bytes...", unlocked_encoded_frame_buffer.len());

        let mut total_read_bytes = 0;

        while total_read_bytes < unlocked_encoded_frame_buffer.len() {
            let read_bytes = self.stream.read(&mut unlocked_encoded_frame_buffer[total_read_bytes..]).unwrap();
            debug!("Received {} bytes", read_bytes); 

            if read_bytes == 0 {
                return Err(ClientError::EmptyFrame);
            }

            total_read_bytes += read_bytes;
        }

        debug!("Total bytes received: {}", total_read_bytes); 

        if total_read_bytes == 0 {
            return Err(ClientError::EmptyFrame);
        }

        Ok(total_read_bytes)
    }
}

#[async_trait]
impl FrameReceiver for TCPFrameReceiver {
    async fn receive_encoded_frame(&mut self) -> Result<usize, ClientError> {
        self.receive_frame_pixels()
    }

    fn get_encoded_frame_buffer(&self)-> Arc<Mutex<Vec<u8>>> {
        self.encoded_frame_buffer.clone()
    }
}