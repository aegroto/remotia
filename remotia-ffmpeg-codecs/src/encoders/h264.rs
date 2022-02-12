#![allow(dead_code)]

use std::{ffi::{CString}, ptr::NonNull, time::Instant};

use log::{debug, info};
use remotia::{
    common::feedback::FeedbackMessage, server::encode::Encoder, traits::FrameProcessor,
    types::FrameData,
};
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avutil::AVDictionary,
    ffi,
};

use async_trait::async_trait;

use cstr::cstr;

use super::{frame_builders::yuv420p::YUV420PAVFrameBuilder, FFMpegEncodingBridge};

#[derive(Default, Debug)]
pub struct H264EncoderState {
    encoded_frames: usize,
    network_stability: f32,
    last_update_network_stability: f32,
}

impl H264EncoderState {
    pub fn increase_network_stability(&mut self, amount: f32) {
        self.network_stability = self.network_stability + amount;
        if self.network_stability > 1.0 {
            self.network_stability = 1.0;
        }
    }

    pub fn decrease_network_stability(&mut self, amount: f32) {
        self.network_stability = self.network_stability - amount;
        if self.network_stability < 0.0 {
            self.network_stability = 0.0;
        }
    }
}

pub struct H264Encoder {
    encode_context: AVCodecContext,

    width: i32,
    height: i32,

    x264opts: CString,

    state: H264EncoderState,

    yuv420_avframe_builder: YUV420PAVFrameBuilder,
    ffmpeg_encoding_bridge: FFMpegEncodingBridge,
}

// TODO: Evaluate a safer way to move the encoder to another thread
// Necessary for multi-threaded pipelines
unsafe impl Send for H264Encoder {}

impl H264Encoder {
    pub fn new(frame_buffer_size: usize, width: i32, height: i32, x264opts: &str) -> Self {
        let x264opts = CString::new(x264opts.to_string()).unwrap();
        let encode_context = init_encoder(width, height, 21, &x264opts);

        H264Encoder {
            width,
            height,

            state: H264EncoderState {
                network_stability: 0.5,
                last_update_network_stability: 0.5,

                ..Default::default()
            },

            x264opts,
            encode_context,

            yuv420_avframe_builder: YUV420PAVFrameBuilder::new(),
            ffmpeg_encoding_bridge: FFMpegEncodingBridge::new(frame_buffer_size),
        }
    }

    fn try_encoder_reconfigure(&mut self) {
        let stability_diff =
            self.state.network_stability - self.state.last_update_network_stability;

        if stability_diff.abs() < 0.1 {
            return;
        }

        let crf = self.recalculate_crf(21, 20);
        info!("Reconfiguring encoder with CRF {}", crf);

        self.encode_context = init_encoder(self.width, self.height, crf, &self.x264opts);
        self.state.last_update_network_stability = self.state.network_stability
    }

    fn recalculate_crf(&mut self, min_crf: u32, max_increase: u32) -> u32 {
        min_crf + (max_increase as f32 * (1.0 - self.state.network_stability)) as u32
    }

    fn perform_quality_increase(&mut self) {
        self.state.increase_network_stability(0.001);
        self.try_encoder_reconfigure();
    }

    fn encode_on_frame_data(&mut self, frame_data: &mut FrameData) {
        let input_buffer = frame_data
            .extract_writable_buffer("raw_frame_buffer")
            .expect("No raw frame buffer in frame DTO");
        let mut output_buffer = frame_data
            .extract_writable_buffer("encoded_frame_buffer")
            .expect("No encoded frame buffer in frame DTO");

        let avframe_building_start_time = Instant::now();
        let avframe = self.yuv420_avframe_builder.create_avframe(
            &mut self.encode_context,
            &input_buffer,
            false,
        );
        frame_data.set(
            "avframe_building_time",
            avframe_building_start_time.elapsed().as_millis(),
        );

        let encoded_bytes = self.ffmpeg_encoding_bridge.encode_avframe(
            &mut self.encode_context,
            avframe,
            &mut output_buffer,
        );

        self.state.encoded_frames += 1;

        debug!("Start: {:?}", &input_buffer[0..16]);

        frame_data.insert_writable_buffer("raw_frame_buffer", input_buffer);
        frame_data.insert_writable_buffer("encoded_frame_buffer", output_buffer);

        frame_data.set("encoded_size", encoded_bytes as u128);
    }
}

fn init_encoder(width: i32, height: i32, crf: u32, x264opts: &CString) -> AVCodecContext {
    let encoder = AVCodec::find_encoder_by_name(cstr!("libx264")).unwrap();
    let mut encode_context = AVCodecContext::new(&encoder);
    encode_context.set_width(width);
    encode_context.set_height(height);
    encode_context.set_time_base(ffi::AVRational { num: 1, den: 60 });
    encode_context.set_framerate(ffi::AVRational { num: 60, den: 1 });
    encode_context.set_pix_fmt(rsmpeg::ffi::AVPixelFormat_AV_PIX_FMT_YUV420P);
    let mut encode_context = unsafe {
        let raw_encode_context = encode_context.into_raw().as_ptr();
        AVCodecContext::from_raw(NonNull::new(raw_encode_context).unwrap())
    };

    let crf_str = format!("{}", crf);
    let crf_str = CString::new(crf_str).unwrap();

    let options = AVDictionary::new(cstr!(""), cstr!(""), 0)
        .set(cstr!("preset"), cstr!("ultrafast"), 0)
        .set(cstr!("crf"), &crf_str, 0)
        .set(cstr!("x264opts"), x264opts, 0)
        .set(cstr!("tune"), cstr!("zerolatency"), 0);

    encode_context.open(Some(options)).unwrap();
    encode_context
}

#[async_trait]
impl FrameProcessor for H264Encoder {
    async fn process(&mut self, mut frame_data: FrameData) -> Option<FrameData> {
        self.encode_on_frame_data(&mut frame_data);
        Some(frame_data)
    }
}

// retro-compatibility for silo pipeline
#[async_trait]
impl Encoder for H264Encoder {
    async fn encode(&mut self, frame_data: &mut FrameData) {
        self.perform_quality_increase();
        self.encode_on_frame_data(frame_data);
    }

    fn handle_feedback(&mut self, message: FeedbackMessage) {
        debug!("Feedback message: {:?}", message);

        match message {
            FeedbackMessage::HighFrameDelay(_) => {
                self.state.decrease_network_stability(0.1);
                debug!("Network stability: {}", self.state.network_stability);
            }
        }

        self.try_encoder_reconfigure();
    }
}
