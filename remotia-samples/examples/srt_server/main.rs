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

    let capture_component = Component::new()
        .add(ScrapFrameCapturer::new_from_primary());

    let pipeline = AscodePipeline::new()
        .add(capture_component);

    pipeline.run().await;

    Ok(())
}
