use log::debug;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVCodecParserContext, AVPacket},
    error::RsmpegError,
};

use cstr::cstr;

use remotia::{
    common::feedback::FeedbackMessage, error::DropReason, traits::FrameProcessor, types::FrameData,
};

use super::{utils::yuv2bgr::raster, Decoder};
use async_trait::async_trait;

pub struct H264Decoder {
    decode_context: AVCodecContext,

    parser_context: AVCodecParserContext,
}

// TODO: Fix all those unsafe impl
unsafe impl Send for H264Decoder {}

impl H264Decoder {
    pub fn new() -> Self {
        let decoder = AVCodec::find_decoder_by_name(cstr!("h264")).unwrap();

        H264Decoder {
            decode_context: {
                let mut decode_context = AVCodecContext::new(&decoder);
                decode_context.open(None).unwrap();

                decode_context
            },

            parser_context: AVCodecParserContext::find(decoder.id).unwrap(),
        }
    }

    fn decoded_yuv_to_bgra(
        &mut self,
        y_frame_buffer: &[u8],
        u_frame_buffer: &[u8],
        v_frame_buffer: &[u8],
        output_buffer: &mut [u8],
    ) {
        raster::yuv_to_bgra_separate(
            y_frame_buffer,
            u_frame_buffer,
            v_frame_buffer,
            output_buffer,
        );
    }

    fn write_avframe(&mut self, avframe: rsmpeg::avutil::AVFrame, output_buffer: &mut [u8]) {
        let data = avframe.data;
        let linesize = avframe.linesize;
        let height = avframe.height as usize;
        let linesize_y = linesize[0] as usize;
        let linesize_cb = linesize[1] as usize;
        let linesize_cr = linesize[2] as usize;
        let y_data = unsafe { std::slice::from_raw_parts_mut(data[0], height * linesize_y) };
        let cb_data = unsafe { std::slice::from_raw_parts_mut(data[1], height / 2 * linesize_cb) };
        let cr_data = unsafe { std::slice::from_raw_parts_mut(data[2], height / 2 * linesize_cr) };
        self.decoded_yuv_to_bgra(y_data, cb_data, cr_data, output_buffer);
    }

    fn parse_packets(&mut self, input_buffer: &[u8]) -> Option<DropReason> {
        let mut packet = AVPacket::new();
        let mut parsed_offset = 0;
        while parsed_offset < input_buffer.len() {
            let (get_packet, offset) = self
                .parser_context
                .parse_packet(
                    &mut self.decode_context,
                    &mut packet,
                    &input_buffer[parsed_offset..],
                )
                .unwrap();

            if get_packet {
                let result = self.decode_context.send_packet(Some(&packet));

                match result {
                    Ok(_) => (),
                    Err(e) => {
                        debug!("Error on send packet: {}", e);
                        return Some(DropReason::FFMpegSendPacketError);
                    }
                }

                packet = AVPacket::new();
            }

            parsed_offset += offset;
        }

        None
    }

    fn decode_to_buffer(
        &mut self,
        input_buffer: &[u8],
        output_buffer: &mut [u8],
    ) -> Result<(), DropReason> {
        if let Some(error) = self.parse_packets(input_buffer) {
            return Err(error);
        }

        let avframe = match self.decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => {
                return Err(DropReason::NoDecodedFrames);
            }
            Err(e) => panic!("{:?}", e),
        };

        self.write_avframe(avframe, output_buffer);

        Ok(())
    }
}

#[async_trait]
impl FrameProcessor for H264Decoder {
    async fn process(&mut self, mut frame_data: FrameData) -> FrameData {
        let mut encoded_frame_buffer = frame_data
            .extract_writable_buffer("encoded_frame_buffer")
            .unwrap();

        let empty_buffer_memory =
            encoded_frame_buffer.split_off(frame_data.get("encoded_size") as usize);

        let mut raw_frame_buffer = frame_data
            .extract_writable_buffer("raw_frame_buffer")
            .unwrap();

        let decode_result = self.decode_to_buffer(&encoded_frame_buffer, &mut raw_frame_buffer);

        encoded_frame_buffer.unsplit(empty_buffer_memory);

        frame_data.insert_writable_buffer("encoded_frame_buffer", encoded_frame_buffer);
        frame_data.insert_writable_buffer("raw_frame_buffer", raw_frame_buffer);

        if let Err(drop_reason) = decode_result {
            frame_data.set_drop_reason(Some(drop_reason));
        }

        frame_data
    }
}

// retro-compatibility with silo pipeline
#[async_trait]
impl Decoder for H264Decoder {
    async fn decode(
        &mut self,
        input_buffer: &[u8],
        output_buffer: &mut [u8],
    ) -> Result<usize, DropReason> {
        match self.decode_to_buffer(input_buffer, output_buffer) {
            Ok(_) => Ok(output_buffer.len()),
            Err(drop_reason) => Err(drop_reason),
        }
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }
}
