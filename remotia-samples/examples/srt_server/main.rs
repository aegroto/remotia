use remotia::{
    common::command_line::parse_canvas_resolution_str,
    server::{
        capture::scrap::ScrapFrameCapturer,
        encode::ffmpeg::h264::H264Encoder,
        profiling::{
            console::{errors::ConsoleServerErrorsProfiler, stats::ConsoleServerStatsProfiler},
            tcp::TCPServerProfiler,
            ServerProfiler,
        },
        send::srt::SRTFrameSender, pipeline::ascode::{AscodePipeline, component::Component},
    },
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    // Pipeline structure
    let pipeline = AscodePipeline::new()
        .add(initialize_capture_component());

    pipeline.run().await;

    Ok(())
}

fn initialize_capture_component() -> Component {
    let capturer = ScrapFrameCapturer::new_from_primary();
    let raw_frame_buffer_size = capturer.width() * capturer.height() * 4;

    let capture_component = Component::new()
        .add(BufferAllocator::new("raw_frame_buffer", raw_frame_buffer_size))
        .add(capturer);


}
