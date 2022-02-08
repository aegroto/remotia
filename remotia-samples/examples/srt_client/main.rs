use remotia::server::pipeline::ascode::{component::Component, AscodePipeline};
use remotia_buffer_utils::BufferAllocator;
use remotia_srt::receiver::SRTFrameReceiver;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Pipeline structure
    let receive_component = initialize_receive_component(1280 * 720 * 4).await;

    let pipeline = AscodePipeline::new()
        .add(receive_component);

    pipeline.run().await;

    Ok(())
}

async fn initialize_receive_component(buffer_size: usize) -> Component {
    let receiver = SRTFrameReceiver::new("127.0.0.1:5001").await;

    Component::new()
        .add(BufferAllocator::new("encoded_frame_buffer", buffer_size))
        .add(receiver)
}
