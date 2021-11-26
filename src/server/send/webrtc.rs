use std::{
    future::Future,
    io::{Cursor, Read, Write},
    net::TcpListener,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;

use bytes::Bytes;
use log::info;
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors,
        media_engine::{MediaEngine, MIME_TYPE_H264},
        APIBuilder,
    },
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    media::{io::h264_reader::H264Reader, Sample},
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
};

use super::FrameSender;

pub struct WebRTCFrameSender {
    video_track: Arc<TrackLocalStaticSample>,
}

impl WebRTCFrameSender {
    pub async fn new() -> Self {
        let mut m = MediaEngine::default();
        m.register_default_codecs().unwrap();
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)
            .await
            .unwrap();

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(config).await.unwrap());

        let notify_tx = Arc::new(tokio::sync::Notify::new());
        let notify_video = notify_tx.clone();

        let (done_tx, _done_rx) = tokio::sync::mpsc::channel::<()>(1);
        let _video_done_tx = done_tx.clone();

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .unwrap();

        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        });

        info!("Printing session info...");

        // Set the handler for ICE connection state
        // This will notify you when the peer has connected/disconnected
        peer_connection
            .on_ice_connection_state_change(Box::new(
                move |connection_state: RTCIceConnectionState| {
                    println!("Connection State has changed {}", connection_state);
                    if connection_state == RTCIceConnectionState::Connected {
                        notify_tx.notify_waiters();
                    }
                    Box::pin(async {})
                },
            ))
            .await;

        // Set the handler for Peer connection state
        // This will notify you when the peer has connected/disconnected
        peer_connection
            .on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
                println!("Peer Connection State has changed: {}", s);

                if s == RTCPeerConnectionState::Failed {
                    // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
                    // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
                    // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
                    println!("Peer Connection has gone to failed exiting");
                    let _ = done_tx.try_send(());
                }

                Box::pin(async {})
            }))
            .await;

        // Exchange offers
        let mut offer_buffer: Vec<u8> = vec![0; 2048];
        let listener = TcpListener::bind("127.0.0.1:5001").unwrap();
        let (mut stream, _client_address) = listener.accept().unwrap();

        info!("Waiting for client offer...");

        let read_bytes = stream.read(&mut offer_buffer).unwrap();
        info!("Read offer bytes: {}", read_bytes);

        let received_b64_offer_buffer = &offer_buffer[..read_bytes];

        // Wait for the offer to be pasted
        // let b64_offer = std::env::var("RDP_SESSION").unwrap();
        let b64_offer = String::from_utf8(received_b64_offer_buffer.to_vec()).unwrap();
        let decoded_b64_offer = base64::decode(b64_offer).unwrap();
        let offer_json_str = String::from_utf8(decoded_b64_offer).unwrap();
        let offer = serde_json::from_str::<RTCSessionDescription>(&offer_json_str).unwrap();

        // Set the remote SessionDescription
        peer_connection.set_remote_description(offer).await.unwrap();

        // Create an answer
        info!("Creating an answer...");
        let answer = peer_connection.create_answer(None).await.unwrap();

        // Create channel that is blocked until ICE Gathering is complete
        info!("Creating channel...");
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        info!("Updating local description and starting UDP listeners...");
        peer_connection.set_local_description(answer).await.unwrap();

        // Block until ICE Gathering is complete, disabling trickle ICE
        // we do this because we only can exchange one signaling message
        // in a production application you should exchange ICE Candidates via OnICECandidate
        info!("Waiting for ICE gathering to complete...");
        let _ = gather_complete.recv().await;

        // Output the answer in base64 so we can paste it in browser
        if let Some(local_desc) = peer_connection.local_description().await {
            let json_str = serde_json::to_string(&local_desc).unwrap();
            let base64_descriptor = base64::encode(json_str);
            stream.write_all(base64_descriptor.as_bytes()).unwrap();
            // println!("{}", base64_descriptor);
        } else {
            println!("generate local_description failed!");
        }

        info!("Waiting for connection...");

        let _ = notify_video.notified().await;

        info!("Connection enstablished");

        Self { video_track }
    }
}

#[async_trait]
impl FrameSender for WebRTCFrameSender {
    async fn send_frame(&mut self, encoded_frame_buffer: &[u8]) {
        info!(
            "Encoded frame buffer ({}) head: {:?}",
            encoded_frame_buffer.len(),
            &encoded_frame_buffer[..16]
        );

        let mut h264_reader = H264Reader::new(Cursor::new(encoded_frame_buffer));

        loop {
            let nal = match h264_reader.next_nal() {
                Ok(nal) => nal,
                Err(_) => {
                    break;
                }
            };

            let sample = Sample {
                data: nal.data.freeze(),
                duration: Duration::from_secs(1),
                ..Default::default()
            };

            self.video_track.write_sample(&sample).await.unwrap();
        }
    }
}
