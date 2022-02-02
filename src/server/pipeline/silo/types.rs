use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
use serde::Serialize;

#[derive(Default, Clone)]
pub struct ServerFrameData {
    readonly_buffers: HashMap<String, Bytes>,
    writable_buffers: HashMap<String, BytesMut>,

    stats: HashMap<String, u128>,
    local_stats: HashMap<String, u128>
}

impl ServerFrameData {
    pub fn set(&mut self, key: String, value: u128) {
        self.stats.insert(key, value);
    }

    pub fn get(&mut self, key: String) -> u128 {
        *self.stats.get(&key).expect("Missing key")
    }

    pub fn set_local(&mut self, key: String, value: u128) {
        self.local_stats.insert(key, value);
    }

    pub fn get_local(&mut self, key: String) -> u128 {
        *self.local_stats.get(&key).expect("Missing key")
    }

    pub fn insert_readonly_buffer(&mut self, key: String, buffer: Bytes) {
        self.readonly_buffers.insert(key, buffer);
    }

    pub fn extract_readonly_buffer(&mut self, key: String) -> Bytes {
        self.readonly_buffers.remove(&key).expect("Missing key")
    }

    pub fn insert_writable_buffer(&mut self, key: String, buffer: BytesMut) {
        self.writable_buffers.insert(key, buffer);
    }

    pub fn extract_writable_buffer(&mut self, key: String) -> BytesMut {
        self.writable_buffers.remove(&key).expect("Missing key")
    }
}
