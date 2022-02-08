use remotia::{
    server::{
        capture::scrap::ScrapFrameCapturer,
        pipeline::ascode::{AscodePipeline, component::Component},
    },
};
use remotia_buffer_utils::BufferAllocator;

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

    Component::new()
        .with_tick(100)
        .add(BufferAllocator::new("raw_frame_buffer", raw_frame_buffer_size))
        .add(capturer)
}
