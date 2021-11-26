use std::{io::{Seek, Write}, sync::{Arc, Mutex}};

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

/// H264Writer is used to take RTP packets, parse them and
/// write the data to an io.Writer.
/// Currently it only supports non-interleaved mode
/// Therefore, only 1-23, 24 (STAP-A), 28 (FU-A) NAL types are allowed.
/// https://tools.ietf.org/html/rfc6184#section-5.2
pub struct RemotiaH264Writer<W: Write + Seek> {
    pub writer: Option<W>,
    pub has_key_frame: bool,
    pub cached_packet: Arc<Mutex<Option<H264Packet>>>,
}

impl<W: Write + Seek> RemotiaH264Writer<W> {
    // new initializes a new H264 writer with an io.Writer output
    pub fn new() -> Self {
        RemotiaH264Writer {
            writer: None,
            has_key_frame: false,
            cached_packet: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_writer(&self) -> &Option<W> {
        &self.writer
    }

    pub fn set_writer(&mut self, writer: W) {
        self.writer = Some(writer);
    }
}

impl<W: Write + Seek> Writer for RemotiaH264Writer<W> {
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

        let mut unlocked_cached_packet = self.cached_packet.lock().unwrap();

        if unlocked_cached_packet.is_none() {
            let _ = unlocked_cached_packet.insert(H264Packet::default());
        }

        let payload = unlocked_cached_packet.as_mut().unwrap().depacketize(&packet.payload)?;

        self.writer.as_mut().unwrap().write_all(&payload)?;

        Ok(())
    }

    /// close closes the underlying writer
    fn close(&mut self) -> Result<(), webrtc_media::Error> {
        let mut unlocked_cached_packet = self.cached_packet.lock().unwrap();
        let _ = unlocked_cached_packet.take();

        self.writer.as_mut().unwrap().flush()?;
        Ok(())
    }
}
