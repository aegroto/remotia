use std::io::Cursor;

use clap::Parser;
use remotia::{
    client::{
        decode::h264::H264Decoder, receive::webrtc::WebRTCFrameReceiver, run_with_configuration,
        ClientConfiguration,
    },
    common::command_line::parse_canvas_resolution_str,
};
use webrtc::media::io::h264_writer::H264Writer;

#[derive(Parser)]
#[clap(version = "0.1.0", author = "Lorenzo C. <aegroto@protonmail.com>")]
struct Options {
    #[clap(short, long, default_value = "1280x720")]
    resolution: String,

    #[clap(short, long, default_value = "127.0.0.1:5001")]
    server_address: String,

    #[clap(short, long, default_value = "5002")]
    binding_port: String,

    #[clap(short, long, default_value = "100")]
    maximum_consecutive_connection_losses: u32,

    #[clap(long)]
    console_profiling: bool,

    #[clap(long)]
    csv_profiling: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let options = Options::parse();
    let (canvas_width, canvas_height) = parse_canvas_resolution_str(&options.resolution);

    let decoder = Box::new(H264Decoder::new(
        canvas_width as usize,
        canvas_height as usize,
    ));

    let frame_receiver = Box::new(
        WebRTCFrameReceiver::new(canvas_width as usize * canvas_height as usize * 3).await,
    );

    run_with_configuration(ClientConfiguration {
        decoder: decoder,
        frame_receiver: frame_receiver,
        canvas_width: canvas_width,
        canvas_height: canvas_height,
        maximum_consecutive_connection_losses: options.maximum_consecutive_connection_losses,
        console_profiling: options.console_profiling,
        csv_profiling: options.csv_profiling,
    })
    .await
}
