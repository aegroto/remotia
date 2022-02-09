use std::time::Duration;

use remotia::server::pipeline::ascode::{component::Component, AscodePipeline};
use remotia_buffer_utils::BufferAllocator;
use remotia_core_capturers::scrap::ScrapFrameCapturer;
use remotia_core_loggers::stats::ConsoleServerStatsProfiler;
use remotia_ffmpeg_codecs::encoders::h264::H264Encoder;
use remotia_profilation_utils::time::{diff::TimestampDiffCalculator, add::TimestampAdder};
use remotia_srt::sender::SRTFrameSender;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Pipeline structure
    let capturer = ScrapFrameCapturer::new_from_primary();
    let width = capturer.width();
    let height = capturer.height();
    let buffer_size = width * height * 4;

    let pipeline = AscodePipeline::new()
        .add(
            Component::new()
                .with_tick(33)
                .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
                .add(TimestampAdder::new("capture_timestamp"))
                .add(capturer),
        )
        .add(
            Component::new()
                .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
                .add(TimestampAdder::new("encoding_start_timestamp"))
                .add(H264Encoder::new(buffer_size, width as i32, height as i32))
                .add(TimestampDiffCalculator::new(
                    "encoding_start_timestamp",
                    "encoding_time",
                )),
        )
        .add(Component::new().add(SRTFrameSender::new(5001, Duration::from_millis(50)).await))
        .add(Component::new().add(ConsoleServerStatsProfiler {
            values_to_log: vec!["encoded_size".to_string(), "encoding_time".to_string()],

            ..Default::default()
        }));

    pipeline.run().await;

    Ok(())
}
