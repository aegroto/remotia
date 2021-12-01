#![allow(dead_code)]

use log::debug;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avutil::{AVDictionary, AVFrame},
    error::RsmpegError,
    ffi,
};

use cstr::cstr;

use crate::server::encode::Encoder;

use super::{frame_builders::yuv420p::YUV420PAVFrameBuilder, FFMpegEncodingBridge};

pub struct H264Encoder {
    encode_context: AVCodecContext,

    width: i32,
    height: i32,

    yuv420_avframe_builder: YUV420PAVFrameBuilder,
    ffmpeg_encoding_bridge: FFMpegEncodingBridge,
}

// TODO: Evaluate a safer way to move the encoder to another thread
// Necessary for multi-threaded pipelines
unsafe impl Send for H264Encoder { }

impl H264Encoder {
    pub fn new(frame_buffer_size: usize, width: i32, height: i32) -> Self {
        H264Encoder {
            width: width,
            height: height,

            encode_context: {
                let encoder = AVCodec::find_encoder_by_name(cstr!("libx264")).unwrap();
                let mut encode_context = AVCodecContext::new(&encoder);

                encode_context.set_width(width);
                encode_context.set_height(height);
                encode_context.set_time_base(ffi::AVRational { num: 1, den: 60 });
                encode_context.set_framerate(ffi::AVRational { num: 60, den: 1 });
                encode_context.set_pix_fmt(rsmpeg::ffi::AVPixelFormat_AV_PIX_FMT_YUV420P);

                let options = AVDictionary::new(cstr!("preset"), cstr!("ultrafast"), 0)
                    .set(cstr!("tune"), cstr!("zerolatency"), 0);

                encode_context.open(Some(options)).unwrap();

                encode_context
            },

            yuv420_avframe_builder: YUV420PAVFrameBuilder::new(width as usize, height as usize),
            ffmpeg_encoding_bridge: FFMpegEncodingBridge::new(frame_buffer_size),
        }
    }
}

impl Encoder for H264Encoder {
    fn encode(&mut self, input_buffer: &[u8], output_buffer: &mut [u8]) -> usize {
        let avframe = self
            .yuv420_avframe_builder
            .create_avframe(&mut self.encode_context, input_buffer);

        self.ffmpeg_encoding_bridge
            .encode_avframe(&mut self.encode_context, avframe, output_buffer)
    }
}
