use remotia::{
    server::pipeline::ascode::{component::Component, AscodePipeline},
};
use remotia_buffer_utils::BufferAllocator;
use remotia_core_renderers::beryllium::BerylliumRenderer;
use remotia_ffmpeg_codecs::decoders::h264::H264Decoder;
use remotia_srt::receiver::SRTFrameReceiver;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let width = 1920;
    let height = 1080;

    // Pipeline structure
    let receive_component = {
        let buffer_size = width * height * 4;
        let receiver = SRTFrameReceiver::new("127.0.0.1:5001").await;

        Component::new()
            .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
            .add(receiver)
    };

    let decode_component = {
        let buffer_size = width * height * 4;
        let decoder = H264Decoder::new();

        Component::new()
            .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
            .add(decoder)
    };

    let render_component = {
        let renderer = BerylliumRenderer::new(width as u32, height as u32);

        Component::new().add(renderer)
    };

    let pipeline = AscodePipeline::new()
        .add(receive_component)
        .add(decode_component)
        .add(render_component);

    pipeline.run().await;

    Ok(())
}
