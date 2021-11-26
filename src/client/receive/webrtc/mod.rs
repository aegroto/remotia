mod h264writer;

use std::{io::{Cursor, Read, Write}, net::{SocketAddr, TcpStream}, str::FromStr, sync::{Arc, Mutex}};

use crate::client::error::ClientError;

use self::h264writer::{RemotiaH264Writer};

use super::FrameReceiver;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use log::info;
use webrtc_media::io::h264_writer;
use std::time::Duration;
use tokio::sync::{mpsc, Notify};
use webrtc::{api::{
        interceptor_registry::register_default_interceptors,
        media_engine::{MediaEngine, MIME_TYPE_H264},
        APIBuilder,
    }, ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer}, interceptor::registry::Registry, media::io::{Writer, h264_writer::H264Writer}, peer_connection::{
        configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription,
    }, rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication, rtp::{codecs::h264::H264Packet, packet::Packet}, rtp_transceiver::{
        rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
        rtp_receiver::RTCRtpReceiver,
    }, track::track_remote::TrackRemote};

struct PacketMessage {
    // payload: Bytes,
    // marker: bool,
    packet: Packet,
}

pub struct WebRTCFrameReceiver {
    packet_receiver: mpsc::Receiver<PacketMessage>,
    // h264_writer: RemotiaH264Writer<Cursor<&'a mut[u8]>>

    pub has_key_frame: bool,
    pub cached_packet: Arc<Mutex<Option<H264Packet>>>,
}

impl WebRTCFrameReceiver {
    pub async fn new() -> WebRTCFrameReceiver {
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
        // let notify_rx = notify_tx.clone();

        // Set a handler for when a new remote track starts, this handler saves buffers to disk as
        // an ivf file, since we could have multiple video tracks we provide a counter.
        // In your application this is where you would handle/process video
        let pc = Arc::downgrade(&peer_connection);

        // let packet_buffer = Arc::new(Mutex::new(vec![0 as u8; 1024]));
        let (packet_sender, packet_receiver) = mpsc::channel(32);

        let receiver = Self {
            packet_receiver: packet_receiver,
            has_key_frame: false,
            cached_packet: Arc::new(Mutex::new(None))
        };

        let peer_connection_packet_sender = packet_sender.clone();

        peer_connection
            .on_track(Box::new(
                move |track: Option<Arc<TrackRemote>>, _receiver: Option<Arc<RTCRtpReceiver>>| {
                    if let Some(track) = track {
                        // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
                        let media_ssrc = track.ssrc();
                        let pc2 = pc.clone();
                        tokio::spawn(async move {
                            let mut result = anyhow::Result::<usize>::Ok(0);
                            while result.is_ok() {
                                let timeout = tokio::time::sleep(Duration::from_secs(3));
                                tokio::pin!(timeout);

                                tokio::select! {
                                    _ = timeout.as_mut() =>{
                                        if let Some(pc) = pc2.upgrade(){
                                            result = pc.write_rtcp(&[Box::new(PictureLossIndication{
                                                sender_ssrc: 0,
                                                media_ssrc,
                                            })]).await.map_err(Into::into);
                                        }else {
                                            break;
                                        }
                                    }
                                };
                            }
                        });

                        // let notify_rx2 = Arc::clone(&notify_rx);

                        let track_writer_packet_sender = peer_connection_packet_sender.clone();
                        Box::pin(async move {
                            let codec = track.codec().await;
                            let mime_type = codec.capability.mime_type.to_lowercase();
                            if mime_type == MIME_TYPE_H264.to_lowercase() {
                                println!("Got h264 track, saving to disk as output.h264");
                                tokio::spawn(async move {
                                    // let _ = save_to_disk(h264_writer2, track, notify_rx2).await;
                                    loop {
                                        let (packet, _attributes) = track.read_rtp().await.unwrap();

                                        // info!("Packet header: {:?} {}", packet.header, packet.payload.len());

                                        match track_writer_packet_sender
                                            .send(PacketMessage {
                                                // payload: packet.payload,
                                                // marker: packet.header.marker
                                                packet: packet,
                                            })
                                            .await
                                        {
                                            Ok(_) => (),
                                            Err(_) => panic!("Unable to send packet message"),
                                        }
                                    }
                                });
                            }
                        })
                    } else {
                        Box::pin(async {})
                    }
                },
            ))
            .await;

        let (done_tx, _done_rx) = mpsc::channel::<()>(1);

        // Set the handler for ICE connection state
        // This will notify you when the peer has connected/disconnected
        peer_connection
            .on_ice_connection_state_change(Box::new(
                move |connection_state: RTCIceConnectionState| {
                    println!("Connection State has changed {}", connection_state);

                    if connection_state == RTCIceConnectionState::Connected {
                        println!("Ctrl+C the remote client to stop the demo");
                    } else if connection_state == RTCIceConnectionState::Failed
                        || connection_state == RTCIceConnectionState::Disconnected
                    {
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

        let mut stream =
            TcpStream::connect(SocketAddr::from_str("127.0.0.1:5001").unwrap()).unwrap();
        let offer_b64_bytes = offer_b64.as_bytes();
        info!("Offer b64 bytes: {}", offer_b64_bytes.len());
        stream.write_all(offer_b64_bytes).unwrap();

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        // peer_connection.set_local_description(answer).await?;

        // Block until ICE Gathering is complete, disabling trickle ICE
        // we do this because we only can exchange one signaling message
        // in a production application you should exchange ICE Candidates via OnICECandidate
        let _ = gather_complete.recv().await;

        info!("Waiting for answer...");

        let mut answer_buffer: Vec<u8> = vec![0; 2048 * 12];
        let read_answer_bytes = stream.read(&mut answer_buffer).unwrap();

        info!("Answer b64 bytes: {}", read_answer_bytes);

        let received_b64_answer_buffer = &answer_buffer[..read_answer_bytes];

        // Wait for the answer to be pasted
        // let b64_answer = std::env::var("RDP_SESSION").unwrap();
        let b64_answer = String::from_utf8(received_b64_answer_buffer.to_vec()).unwrap();
        let decoded_b64_answer = base64::decode(b64_answer).unwrap();
        let answer_json_str = String::from_utf8(decoded_b64_answer).unwrap();
        let answer = serde_json::from_str::<RTCSessionDescription>(&answer_json_str).unwrap();

        peer_connection
            .set_remote_description(answer)
            .await
            .unwrap();

        // Output the answer in base64 so we can paste it in browser
        /*if let Some(local_desc) = peer_connection.local_description().await {
            let json_str = serde_json::to_string(&local_desc).unwrap();
            let b64 = base64::encode(&json_str);
            println!("{}", b64);
        } else {
            println!("generate local_description failed!");
        }*/

        receiver
    }
}

#[async_trait]
impl FrameReceiver for WebRTCFrameReceiver {
    async fn receive_encoded_frame(
        &mut self,
        encoded_frame_buffer: &mut[u8],
    ) -> Result<usize, ClientError> {
        // let mut cursor = Cursor::new(encoded_frame_buffer);

        let cursor = Cursor::new(encoded_frame_buffer);
        let mut h264_writer = RemotiaH264Writer::new();
        h264_writer.set_writer(cursor);

        h264_writer.has_key_frame = self.has_key_frame;
        h264_writer.cached_packet = self.cached_packet.clone();

        loop {
            let received_message = self.packet_receiver.recv().await;

            if let Some(packet_message) = received_message {
                let packet = packet_message.packet;

                h264_writer.write_rtp(&packet).unwrap();

                if packet.header.marker {
                    break;
                }
            }
        }

        let wrote_bytes = h264_writer.get_writer().as_ref().unwrap().position() as usize;

        h264_writer.close().unwrap();

        self.has_key_frame = h264_writer.has_key_frame;
        self.cached_packet = h264_writer.cached_packet.clone();

        Ok(wrote_bytes)
    }
}
