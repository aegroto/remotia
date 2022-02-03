use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
use serde::Serialize;

use crate::server::error::ServerError;

#[derive(Default, Clone, Debug)]
pub struct ServerFrameData {
    readonly_buffers: HashMap<String, Bytes>,
    writable_buffers: HashMap<String, BytesMut>,

    stats: HashMap<String, u128>,
    local_stats: HashMap<String, u128>,

    error: Option<ServerError>
}

impl ServerFrameData {
    pub fn set(&mut self, key: &str, value: u128) {
        self.stats.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> u128 {
        *self.stats.get(key).expect("Missing key")
    }

    pub fn has(&self, key: &str) -> bool {
        self.stats.contains_key(key)
    }

    pub fn set_local(&mut self, key: &str, value: u128) {
        self.local_stats.insert(key.to_string(), value);
    }

    pub fn get_local(&self, key: &str) -> u128 {
        *self.local_stats.get(key).expect(&missing_key_msg(key))
    }

    pub fn has_local(&self, key: &str) -> bool {
        self.local_stats.contains_key(key)
    }

    pub fn insert_readonly_buffer(&mut self, key: &str, buffer: Bytes) {
        self.readonly_buffers.insert(key.to_string(), buffer);
    }

    pub fn extract_readonly_buffer(&mut self, key: &str) -> Bytes {
        self.readonly_buffers.remove(key).expect(&missing_key_msg(key))
    }

    pub fn get_readonly_buffer_ref(&mut self, key: &str) -> &Bytes {
        self.readonly_buffers.get(key).expect(&missing_key_msg(key))
    }

    pub fn insert_writable_buffer(&mut self, key: &str, buffer: BytesMut) {
        self.writable_buffers.insert(key.to_string(), buffer);
    }

    pub fn extract_writable_buffer(&mut self, key: &str) -> BytesMut {
        self.writable_buffers.remove(key).expect(&missing_key_msg(key))
    }

    pub fn get_writable_buffer_ref(&mut self, key: &str) -> &mut BytesMut {
        self.writable_buffers.get_mut(key).expect(&missing_key_msg(key))
    }

    pub fn set_error(&mut self, error: Option<ServerError>) {
        self.error = error;
    }

    pub fn get_error(&self) -> Option<ServerError> {
        self.error
    }
}

fn missing_key_msg(key: &str) -> String {
    format!("Missing key '{}'", key)
}