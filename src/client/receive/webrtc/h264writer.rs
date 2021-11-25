// Modified version of H264Writer from WebRTC.rs to allow wrote buffer read-only access

use std::{io::{Cursor, Seek, Write}, sync::{Arc, Mutex}};

use bytes::Bytes;
use thiserror::Error;
use webrtc::{media::io::Writer, rtp::{self, codecs::h264::H264Packet, packetizer::Depacketizer}};

const NALU_TTYPE_STAP_A: u32 = 24;
const NALU_TTYPE_SPS: u32 = 7;
const NALU_TYPE_BITMASK: u32 = 0x1F;

fn is_key_frame(data: &[u8]) -> bool {
    if data.len() < 4 {
        false
    } else {
        let word = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let nalu_type = (word >> 24) & NALU_TYPE_BITMASK;
        (nalu_type == NALU_TTYPE_STAP_A && (word & NALU_TYPE_BITMASK) == NALU_TTYPE_SPS)
            || (nalu_type == NALU_TTYPE_SPS)
    }
}

type W = Arc<Mutex<Vec<u8>>>;

pub struct RemotiaH264Writer {
    writer: W,
    has_key_frame: bool,
    cached_packet: Option<H264Packet>,
}

impl RemotiaH264Writer {
    // new initializes a new H264 writer with an io.Writer output
    pub fn new(writer: W) -> Self {
        RemotiaH264Writer {
            writer,
            has_key_frame: false,
            cached_packet: None,
        }
    }

    pub fn get_writer(&self) -> &W {
        &self.writer
    }
}

impl Writer for RemotiaH264Writer {
    /// write_rtp adds a new packet and writes the appropriate headers for it
    fn write_rtp(&mut self, packet: &rtp::packet::Packet) -> Result<(), webrtc_media::Error> {
        if packet.payload.is_empty() {
            return Ok(());
        }

        if !self.has_key_frame {
            self.has_key_frame = is_key_frame(&packet.payload);
            if !self.has_key_frame {
                // key frame not defined yet. discarding packet
                return Ok(());
            }
        }

        if self.cached_packet.is_none() {
            self.cached_packet = Some(H264Packet::default());
        }

        if let Some(cached_packet) = &mut self.cached_packet {
            let payload = cached_packet.depacketize(&packet.payload)?;

            let mut unlocked_writer = self.writer.lock().unwrap();
            unlocked_writer.write_all(&payload)?;
        }

        Ok(())
    }

    /// close closes the underlying writer
    fn close(&mut self) -> Result<(), webrtc_media::Error> {
        self.cached_packet = None;
        let mut unlocked_writer = self.writer.lock().unwrap();
        unlocked_writer.flush()?;
        Ok(())
    }
}
