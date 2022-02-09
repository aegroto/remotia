use remotia::server::pipeline::ascode::{component::Component, AscodePipeline};
use remotia_buffer_utils::BufferAllocator;
use remotia_ffmpeg_codecs::decoders::h264::H264Decoder;
use remotia_srt::receiver::SRTFrameReceiver;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let width = 1920;
    let height = 1080;

    // Pipeline structure
    let receive_component = initialize_receive_component(width * height * 4).await;
    let decode_component = initialize_decode_component(width * height * 4);

    let pipeline = AscodePipeline::new()
        .add(receive_component)
        .add(decode_component);

    pipeline.run().await;

    Ok(())
}

async fn initialize_receive_component(buffer_size: usize) -> Component {
    let receiver = SRTFrameReceiver::new("127.0.0.1:5001").await;

    Component::new()
        .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
        .add(receiver)
}

fn initialize_decode_component(buffer_size: usize) -> Component {
    let decoder = H264Decoder::new();

    Component::new()
        .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
        .add(decoder)
}
