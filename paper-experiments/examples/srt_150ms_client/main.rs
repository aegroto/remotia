use std::time::Duration;

use remotia::{
    error::DropReason,
    processors::{error_switch::OnErrorSwitch, frame_drop::threshold::ThresholdBasedFrameDropper, key_check::KeyChecker, ticker::Ticker},
    pipeline::ascode::{component::Component, AscodePipeline},
};
use remotia_buffer_utils::pool::BuffersPool;
use remotia_core_loggers::{
    csv::serializer::CSVFrameDataSerializer, errors::ConsoleDropReasonLogger,
    stats::ConsoleAverageStatsLogger,
};
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

    let efb_pool = BuffersPool::new("encoded_frame_buffer", 8, buffer_size);
    let rfb_pool = BuffersPool::new("raw_frame_buffer", 8, buffer_size);

    let error_handling_pipeline = AscodePipeline::new()
        .tag("ErrorsHandler")
        .link(
            Component::new()
                .add(rfb_pool.redeemer().soft())
                .add(efb_pool.redeemer().soft())

                .add(
                    ConsoleDropReasonLogger::new()
                        .log(DropReason::StaleFrame)
                        .log(DropReason::ConnectionError)
                        .log(DropReason::CodecError)
                        .log(DropReason::NoDecodedFrames)
                        .log(DropReason::ConnectionError)
                        .log(DropReason::NoAvailableBuffers),
                )
                .add(KeyChecker::new("capture_timestamp"))
                .add(CSVFrameDataSerializer::new("client_drops.csv").log("capture_timestamp")),
        )
        .bind()
        .feedable();

    // Pipeline structure
    let main_pipeline = AscodePipeline::new()
        .tag("ClientMain")
        .link(
            Component::new()
                .add(Ticker::new(10))
                .add(efb_pool.borrower())
                .add(OnErrorSwitch::new(&error_handling_pipeline))

                .add(TimestampAdder::new("reception_start_timestamp"))
                .add(SRTFrameReceiver::new("127.0.0.1:5001", Duration::from_millis(150)).await)
                .add(TimestampDiffCalculator::new(
                    "reception_start_timestamp",
                    "reception_time",
                ))
                .add(OnErrorSwitch::new(&error_handling_pipeline)),
        )
        .link(
            Component::new()
                .add(rfb_pool.borrower())
                .add(OnErrorSwitch::new(&error_handling_pipeline))

                .add(TimestampAdder::new("decoding_start_timestamp"))
                .add(H264Decoder::new())
                // .add(H265Decoder::new())
                // .add(LibVpxVP9Decoder::new())
                .add(TimestampDiffCalculator::new(
                    "decoding_start_timestamp",
                    "decoding_time",
                ))
                .add(efb_pool.redeemer())
                .add(OnErrorSwitch::new(&error_handling_pipeline)),
        )
        .link(
            Component::new()
                .add(TimestampDiffCalculator::new(
                    "capture_timestamp",
                    "pre_render_frame_delay",
                ))
                .add(ThresholdBasedFrameDropper::new(
                    "pre_render_frame_delay",
                    300,
                ))
                .add(OnErrorSwitch::new(&error_handling_pipeline))
                .add(TimestampAdder::new("rendering_start_timestamp"))
                .add(BerylliumRenderer::new(width as u32, height as u32))
                .add(TimestampDiffCalculator::new(
                    "rendering_start_timestamp",
                    "rendering_time",
                ))

                .add(rfb_pool.redeemer())
                .add(OnErrorSwitch::new(&error_handling_pipeline))

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
                )
                .add(
                    CSVFrameDataSerializer::new("client.csv")
                        .log("capture_timestamp")
                        .log("reception_time")
                        .log("decoding_time")
                        .log("rendering_time")
                        .log("total_time")
                        .log("reception_delay")
                        .log("frame_delay"),
                ),
        )
        .bind();

    let mut handles = Vec::new();
    handles.extend(main_pipeline.run());
    handles.extend(error_handling_pipeline.run());

    for handle in handles {
        handle.await.unwrap()
    }

    Ok(())
}
