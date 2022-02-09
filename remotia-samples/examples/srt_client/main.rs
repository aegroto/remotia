use remotia::server::pipeline::ascode::{component::Component, AscodePipeline};
use remotia_buffer_utils::BufferAllocator;
use remotia_core_loggers::stats::ConsoleAverageStatsLogger;
use remotia_core_renderers::beryllium::BerylliumRenderer;
use remotia_ffmpeg_codecs::decoders::h264::H264Decoder;
use remotia_profilation_utils::time::{add::TimestampAdder, diff::TimestampDiffCalculator};
use remotia_srt::receiver::SRTFrameReceiver;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let width = 1280;
    let height = 720;
    let buffer_size = width * height * 4;

    // Pipeline structure
    let pipeline = AscodePipeline::new()
        .add(
            Component::new()
                .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
                .add(TimestampAdder::new("reception_start_timestamp"))
                .add(SRTFrameReceiver::new("127.0.0.1:5001").await)
                .add(TimestampDiffCalculator::new(
                    "reception_start_timestamp",
                    "reception_time",
                )),
        )
        .add(
            Component::new()
                .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
                .add(TimestampAdder::new("decoding_start_timestamp"))
                .add(H264Decoder::new())
                .add(TimestampDiffCalculator::new(
                    "decoding_start_timestamp",
                    "decoding_time",
                )),
        )
        .add(
            Component::new()
                .add(TimestampAdder::new("rendering_start_timestamp"))
                .add(BerylliumRenderer::new(width as u32, height as u32))
                .add(TimestampDiffCalculator::new(
                    "rendering_start_timestamp",
                    "rendering_time",
                )),
        )
        .add(Component::new().add(ConsoleAverageStatsLogger {
            values_to_log: vec![
                "reception_time".to_string(),
                "decoding_time".to_string(),
                "rendering_time".to_string(),
            ],

            ..Default::default()
        }))
        .bind();

    pipeline.run().await;

    Ok(())
}
