use std::{net::SocketAddr, str::FromStr};

use clap::Parser;
use remotia::{
    client::{run_with_configuration, ClientConfiguration},
    common::command_line::parse_canvas_resolution_str,
};
use utils::setup_frame_receiver_by_name;

use crate::utils::setup_decoder_from_name;

mod utils;

#[derive(Parser)]
#[clap(version = "0.1.0", author = "Lorenzo C. <aegroto@protonmail.com>")]
struct Options {
    #[clap(short, long, default_value = "h264rgb")]
    decoder_name: String,

    #[clap(short, long, default_value = "tcp")]
    frame_receiver_name: String,

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let options = Options::parse();
    let (canvas_width, canvas_height) = parse_canvas_resolution_str(&options.resolution);

    run_with_configuration(ClientConfiguration {
        decoder: setup_decoder_from_name(canvas_width, canvas_height, &options.decoder_name),
        frame_receiver: setup_frame_receiver_by_name(
            SocketAddr::from_str(&options.server_address)?,
            &options.binding_port,
            &options.frame_receiver_name,
        )?,
        canvas_width: canvas_width,
        canvas_height: canvas_height,
        maximum_consecutive_connection_losses: options.maximum_consecutive_connection_losses,
        console_profiling: options.console_profiling,
        csv_profiling: options.csv_profiling,
    })
}
