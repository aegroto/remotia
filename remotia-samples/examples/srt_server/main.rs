use remotia::server::pipeline::ascode::{component::Component, AscodePipeline};
use remotia_buffer_utils::BufferAllocator;
use remotia_core_capturers::scrap::ScrapFrameCapturer;
use remotia_ffmpeg_codecs::h264::H264Encoder;
use remotia_srt::sender::SRTFrameSender;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    // Pipeline structure

    let (width, height, capture_component) = initialize_capture_component();
    let encode_component = initialize_encode_component(width, height);
    let transmission_component = initialize_transmission_component().await;

    let pipeline = AscodePipeline::new()
        .add(capture_component)
        .add(encode_component)
        .add(transmission_component);

    pipeline.run().await;

    Ok(())
}

async fn initialize_transmission_component() -> Component {
    let sender = SRTFrameSender::new(5001).await;

    Component::new().add(sender)
}

fn initialize_encode_component(width: usize, height: usize) -> Component {
    let buffer_size = width * height * 4;
    let encoder = H264Encoder::new(buffer_size, width as i32, height as i32);

    Component::new()
        .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
        .add(encoder)
}

fn initialize_capture_component() -> (usize, usize, Component) {
    let capturer = ScrapFrameCapturer::new_from_primary();
    let width = capturer.width();
    let height = capturer.height();
    let buffer_size = width * height * 4;

    let component = Component::new()
        .with_tick(100)
        .add(BufferAllocator::new("raw_frame_buffer", buffer_size))
        .add(capturer);

    (width, height, component)
}
