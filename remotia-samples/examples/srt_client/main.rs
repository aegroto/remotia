use std::time::Duration;

use remotia::{
    processors::{error_switch::OnErrorSwitch, frame_drop::ThresholdBasedFrameDropper},
    server::pipeline::ascode::{component::Component, AscodePipeline},
};
use remotia_buffer_utils::BufferAllocator;
use remotia_core_loggers::{printer::ConsoleFrameDataPrinter, stats::ConsoleAverageStatsLogger};
use remotia_core_renderers::beryllium::BerylliumRenderer;
use remotia_ffmpeg_codecs::decoders::h264::H264Decoder;
use remotia_profilation_utils::time::{add::TimestampAdder, diff::TimestampDiffCalculator};
use remotia_srt::receiver::SRTFrameReceiver;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let error_handling_pipeline = AscodePipeline::new()
        .tag("ErrorsHandler")
        .link(Component::new().add(ConsoleFrameDataPrinter::new()))
        .bind()
        .feedable();

    let width = 1280;
    let height = 720;
    let buffer_size = width * height * 4;

    // Pipeline structure
    let main_pipeline = AscodePipeline::new()
        .tag("ClientMain")
        .link(
            Component::new()
                .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
                .add(TimestampAdder::new("reception_start_timestamp"))
                .add(SRTFrameReceiver::new("127.0.0.1:5001", Duration::from_millis(50)).await)
                .add(TimestampDiffCalculator::new(
                    "reception_start_timestamp",
                    "reception_time",
                ))
                .add(ThresholdBasedFrameDropper::new("reception_time", 10))
                .add(OnErrorSwitch::new(&error_handling_pipeline)),
        )
        .link(
            Component::new()
                .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
                .add(TimestampAdder::new("decoding_start_timestamp"))
                .add(H264Decoder::new())
                .add(TimestampDiffCalculator::new(
                    "decoding_start_timestamp",
                    "decoding_time",
                ))
                .add(OnErrorSwitch::new(&error_handling_pipeline)),
        )
        .link(
            Component::new()
                .add(TimestampAdder::new("rendering_start_timestamp"))
                .add(BerylliumRenderer::new(width as u32, height as u32))
                .add(TimestampDiffCalculator::new(
                    "rendering_start_timestamp",
                    "rendering_time",
                ))
                .add(TimestampDiffCalculator::new(
                    "reception_start_timestamp",
                    "total_time",
                ))
                .add(TimestampDiffCalculator::new(
                    "capture_timestamp",
                    "frame_delay",
                ))
                .add(OnErrorSwitch::new(&error_handling_pipeline)),
        )
        .link(
            Component::new()
                .add(
                    ConsoleAverageStatsLogger::new()
                        .header("--- Computational times")
                        .log("reception_time")
                        .log("decoding_time")
                        .log("rendering_time")
                        .log("total_time"),
                )
                .add(
                    ConsoleAverageStatsLogger::new()
                        .header("--- Delay times")
                        .log("reception_delay")
                        .log("frame_delay"),
                ),
        )
        .bind();

    let main_handle = main_pipeline.run();
    let error_handle = error_handling_pipeline.run();

    main_handle.await.unwrap();
    error_handle.await.unwrap();

    Ok(())
}
