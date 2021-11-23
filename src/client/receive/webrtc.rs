use std::{io::Write, net::{SocketAddr, TcpStream}, str::FromStr, sync::Arc};

use crate::client::error::ClientError;

use super::FrameReceiver;

use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::Notify;
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors,
        media_engine::{MediaEngine, MIME_TYPE_H264},
        APIBuilder,
    },
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription,
    },
    rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication,
    rtp_transceiver::{
        rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
        rtp_receiver::RTCRtpReceiver,
    },
    track::track_remote::TrackRemote,
};

pub struct WebRTCFrameReceiver {}

impl WebRTCFrameReceiver {
    pub async fn new() -> Self {
        // Create a MediaEngine object to configure the supported codec
        let mut m = MediaEngine::default();

        // Setup the codecs you want to use.
        // We'll use a H264 and Opus but you can also define your own
        m.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: 90000,
                    channels: 0,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                payload_type: 102,
                ..Default::default()
            },
            RTPCodecType::Video,
        )
        .unwrap();

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
        // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
        // for each PeerConnection.
        let mut registry = Registry::new();

        // Use the default set of Interceptors
        registry = register_default_interceptors(registry, &mut m)
            .await
            .unwrap();

        // Create the API object with the MediaEngine
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // Prepare the configuration
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = Arc::new(api.new_peer_connection(config).await.unwrap());

        // Allow us to receive 1 audio track, and 1 video track
        peer_connection
            .add_transceiver_from_kind(RTPCodecType::Video, &[])
            .await
            .unwrap();

        let notify_tx = Arc::new(Notify::new());
        let notify_rx = notify_tx.clone();

        // Set a handler for when a new remote track starts, this handler saves buffers to disk as
        // an ivf file, since we could have multiple video tracks we provide a counter.
        // In your application this is where you would handle/process video
        let pc = Arc::downgrade(&peer_connection);
        peer_connection
            .on_track(Box::new(
                move |track: Option<Arc<TrackRemote>>, _receiver: Option<Arc<RTCRtpReceiver>>| {
                    if let Some(track) = track {
                        // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
                        let media_ssrc = track.ssrc();
                        let pc2 = pc.clone();
                        tokio::spawn(async move {
                            loop {
                                let timeout = tokio::time::sleep(Duration::from_secs(3));
                                tokio::pin!(timeout);

                                tokio::select! {
                                    _ = timeout.as_mut() =>{
                                        if let Some(pc) = pc2.upgrade(){
                                            pc.write_rtcp(&[Box::new(PictureLossIndication{
                                                sender_ssrc: 0,
                                                media_ssrc,
                                            })]).await;
                                        } else {
                                            break;
                                        }
                                    }
                                };
                            }
                        });

                        let notify_rx2 = Arc::clone(&notify_rx);
                        Box::pin(async move {
                            let codec = track.codec().await;
                            let mime_type = codec.capability.mime_type.to_lowercase();
                            if mime_type == MIME_TYPE_H264.to_lowercase() {
                                println!("Got h264 track, saving to disk as output.h264");
                                tokio::spawn(async move {
                                    // let _ = save_to_disk(h264_writer2, track, notify_rx2).await;
                                });
                            }
                        })
                    } else {
                        Box::pin(async {})
                    }
                },
            ))
            .await;

        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Set the handler for ICE connection state
        // This will notify you when the peer has connected/disconnected
        peer_connection
            .on_ice_connection_state_change(Box::new(
                move |connection_state: RTCIceConnectionState| {
                    println!("Connection State has changed {}", connection_state);

                    if connection_state == RTCIceConnectionState::Connected {
                        println!("Ctrl+C the remote client to stop the demo");
                    } else if connection_state == RTCIceConnectionState::Failed {
                        notify_tx.notify_waiters();

                        println!("Done writing media files");

                        let _ = done_tx.try_send(());
                    }
                    Box::pin(async {})
                },
            ))
            .await;

        // Wait for the offer to be pasted
        /*let line = signal::must_read_stdin()?;
        let desc_data = signal::decode(line.as_str())?;
        let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

        // Set the remote SessionDescription
        peer_connection.set_remote_description(offer).await?;*/

        let offer = peer_connection.create_offer(None).await.unwrap();
        let offer_json = serde_json::to_string::<RTCSessionDescription>(&offer).unwrap();
        peer_connection.set_local_description(offer).await.unwrap();
        let offer_b64 = base64::encode(offer_json);

        let mut stream = TcpStream::connect(SocketAddr::from_str("127.0.0.1:5001").unwrap()).unwrap();
        stream.write_all(offer_b64.as_bytes()).unwrap();

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        // peer_connection.set_local_description(answer).await?;

        // Block until ICE Gathering is complete, disabling trickle ICE
        // we do this because we only can exchange one signaling message
        // in a production application you should exchange ICE Candidates via OnICECandidate
        let _ = gather_complete.recv().await;

        // Output the answer in base64 so we can paste it in browser
        if let Some(local_desc) = peer_connection.local_description().await {
            let json_str = serde_json::to_string(&local_desc).unwrap();
            let b64 = base64::encode(&json_str);
            println!("{}", b64);
        } else {
            println!("generate local_description failed!");
        }
        Self {}
    }
}

#[async_trait]
impl FrameReceiver for WebRTCFrameReceiver {
    async fn receive_encoded_frame(
        &mut self,
        frame_buffer: &mut [u8],
    ) -> Result<usize, ClientError> {
        Ok(0)
    }
}
