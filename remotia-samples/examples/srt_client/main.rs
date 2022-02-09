use remotia::server::pipeline::ascode::{component::Component, AscodePipeline};
use remotia_buffer_utils::BufferAllocator;
use remotia_core_renderers::beryllium::BerylliumRenderer;
use remotia_ffmpeg_codecs::decoders::h264::H264Decoder;
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
                .add(SRTFrameReceiver::new("127.0.0.1:5001").await),
        )
        .add(
            Component::new()
                .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
                .add(H264Decoder::new()),
        )
        .add(Component::new().add(BerylliumRenderer::new(width as u32, height as u32)));

    pipeline.run().await;

    Ok(())
}
