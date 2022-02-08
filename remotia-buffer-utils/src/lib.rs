use remotia::server::{traits::FrameProcessor, types::ServerFrameData};

pub struct BufferAllocator { 
    buffer_id: String,
    size: usize
}

impl BufferAllocator {
    pub fn new(buffer_id: &str, size: usize) -> Self {
        Self {
            buffer_id,
            size
        }
    }

    fn allocate_buffer(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.size);
        buf.resize(size, 0);
        buf
    }
}

#[async_trait]
impl FrameProcessor for BufferAllocator {
    async fn process(&mut self, frame_data: ServerFrameData) -> ServerFrameData {
        frame_data.insert_writable_buffer(self.buffer_id, self.allocate_buffer());
        frame_data
    }

}