use std::env;
use std::net::TcpStream;

mod decode;
mod profile;
mod receive;
mod render;

use beryllium::{
  event::Event,
  gl_window::{GlAttr, GlContextFlags, GlProfile},
  init::{InitFlags, Sdl},
  window::WindowFlags,
  SdlResult,
};

use std::net::SocketAddr;
use std::net::UdpSocket;
use std::ops::ControlFlow;
use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use beryllium::*;

use bytes::BytesMut;
use chrono::Utc;
use clap::Parser;
use log::info;
use log::{debug, error, warn};
use pixels::wgpu;
use pixels::PixelsBuilder;
use pixels::{wgpu::Surface, Pixels, SurfaceTexture};
use tokio::sync::mpsc;
use zstring::zstr;

use crate::client::decode::Decoder;
use crate::client::error::ClientError;
use crate::client::pipeline::silo::decode::launch_decode_thread;
use crate::client::pipeline::silo::decode::DecodeResult;
use crate::client::pipeline::silo::profile::launch_profile_thread;
use crate::client::pipeline::silo::receive::launch_receive_thread;
use crate::client::pipeline::silo::receive::ReceiveResult;
use crate::client::pipeline::silo::render::launch_render_thread;
use crate::client::pipeline::silo::render::RenderResult;
use crate::client::profiling::logging::console::ReceptionRoundConsoleLogger;
use crate::client::profiling::logging::csv::ReceptionRoundCSVLogger;
use crate::client::profiling::ReceivedFrameStats;
use crate::client::profiling::ReceptionRoundStats;
use crate::client::receive::FrameReceiver;
use crate::client::utils::decoding::packed_bgr_to_packed_rgba;
use crate::client::utils::profilation::setup_round_stats;

pub struct SiloClientConfiguration {
    pub decoder: Box<dyn Decoder + Send>,
    pub frame_receiver: Box<dyn FrameReceiver + Send>,

    pub canvas_width: u32,
    pub canvas_height: u32,

    pub maximum_consecutive_connection_losses: u32,

    pub target_fps: u32,

    pub console_profiling: bool,
    pub csv_profiling: bool,
}

pub struct SiloClientPipeline {
    config: SiloClientConfiguration,
}

impl SiloClientPipeline {
    pub fn new(config: SiloClientConfiguration) -> Self {
        Self { config }
    }

    pub async fn run(self) {
        // Init display
        let sdl = Sdl::init(InitFlags::EVERYTHING).unwrap();
        sdl.allow_drop_events(true);

        const FLAGS: i32 = if cfg!(debug_assertions) {
            GlContextFlags::FORWARD_COMPATIBLE.as_i32() | GlContextFlags::DEBUG.as_i32()
        } else {
            GlContextFlags::FORWARD_COMPATIBLE.as_i32()
        };
        sdl.gl_set_attribute(GlAttr::MajorVersion, 3).unwrap();
        sdl.gl_set_attribute(GlAttr::MinorVersion, 3).unwrap();
        sdl.gl_set_attribute(GlAttr::Profile, GlProfile::Core as _).unwrap();
        sdl.gl_set_attribute(GlAttr::Flags, FLAGS).unwrap();

        let gl_win = sdl.create_gl_window(
            zstr!("GL Demo Window"),
            None,
            (8000000, 6000000),
            WindowFlags::ALLOW_HIGHDPI,
        ).unwrap();
        gl_win.set_swap_interval(1).unwrap();

        let window = &*gl_win;

        info!("Starting to receive stream...");

        const MAXIMUM_ENCODED_FRAME_BUFFERS: usize = 16;
        const MAXIMUM_RAW_FRAME_BUFFERS: usize = 64;

        let raw_frame_size = (self.config.canvas_width * self.config.canvas_height * 3) as usize;
        let maximum_encoded_frame_size =
            (self.config.canvas_width * self.config.canvas_height * 3) as usize;

        let (encoded_frame_buffers_sender, encoded_frame_buffers_receiver) =
            mpsc::unbounded_channel::<BytesMut>();
        let (raw_frame_buffers_sender, raw_frame_buffers_receiver) =
            mpsc::unbounded_channel::<BytesMut>();

        for _ in 0..MAXIMUM_ENCODED_FRAME_BUFFERS {
            let mut buf = BytesMut::with_capacity(maximum_encoded_frame_size);
            buf.resize(maximum_encoded_frame_size, 0);
            encoded_frame_buffers_sender.send(buf).unwrap();
        }

        for _ in 0..MAXIMUM_RAW_FRAME_BUFFERS {
            let mut buf = BytesMut::with_capacity(raw_frame_size);
            buf.resize(raw_frame_size, 0);
            raw_frame_buffers_sender.send(buf).unwrap();
        }

        let pixels = {
            let surface_texture =
                SurfaceTexture::new(self.config.canvas_width, self.config.canvas_height, &window);
            PixelsBuilder::new(
                self.config.canvas_width,
                self.config.canvas_height,
                surface_texture,
            )
            .build()
            .unwrap()
        };

        let (receive_result_sender, receive_result_receiver) =
            mpsc::unbounded_channel::<ReceiveResult>();
        let (decode_result_sender, decode_result_receiver) =
            mpsc::unbounded_channel::<DecodeResult>();
        let (render_result_sender, render_result_receiver) =
            mpsc::unbounded_channel::<RenderResult>();

        let receive_handle = launch_receive_thread(
            self.config.frame_receiver,
            encoded_frame_buffers_receiver,
            receive_result_sender,
        );

        let decode_handle = launch_decode_thread(
            self.config.decoder,
            raw_frame_buffers_receiver,
            encoded_frame_buffers_sender,
            receive_result_receiver,
            decode_result_sender,
        );

        let render_handle = launch_render_thread(
            self.config.target_fps,
            pixels,
            raw_frame_buffers_sender,
            decode_result_receiver,
            render_result_sender,
        );

        let profile_handle = launch_profile_thread(
            render_result_receiver,
            self.config.csv_profiling,
            self.config.console_profiling,
        );

        receive_handle.await.unwrap();
        decode_handle.await.unwrap();
        render_handle.await.unwrap();
        profile_handle.await.unwrap();
    }
}
