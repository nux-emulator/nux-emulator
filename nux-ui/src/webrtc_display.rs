//! WebRTC display client for Cuttlefish (experimental, NUX_WEBRTC=1).
//!
//! Connects to the Cuttlefish WebRTC signaling server (port 8443),
//! negotiates a WebRTC session, and renders the video stream via
//! GStreamer: `webrtcbin → rtpvp9depay → vp9dec → videoconvert → appsink`.

#![allow(dead_code)]
//!
//! Cuttlefish signaling protocol (polling mode):
//!   1. POST /connect       {device_id}              → {connection_id}
//!   2. POST /forward       {connection_id, payload}  → [device_messages]
//!   3. POST /poll_messages  {connection_id}           → [device_messages]
//!
//! The DEVICE creates the SDP offer, the CLIENT creates the answer.

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_sdp as gst_sdp;
use gstreamer_video as gst_video;
use gstreamer_webrtc as gst_webrtc;
use gtk::glib;
use gtk4 as gtk;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Decoded video frame from WebRTC appsink.
struct WebRtcFrame {
    width: u32,
    height: u32,
    stride: u32,
    data: Vec<u8>,
}

// ── Signaling ──

const SIGNALING_URL: &str = "https://localhost:8443";
const DEVICE_ID: &str = "cvd-1";

/// TLS-ignoring HTTP agent for Cuttlefish's self-signed cert.
fn make_agent() -> ureq::Agent {
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .expect("TLS connector");
    ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .build()
}

/// POST JSON helper — returns parsed JSON response.
fn post_json(
    agent: &ureq::Agent,
    url: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    agent
        .post(url)
        .send_json(body)
        .map_err(|e| format!("POST {url}: {e}"))?
        .into_json::<serde_json::Value>()
        .map_err(|e| format!("parse {url}: {e}"))
}

/// Connect to the signaling server, returns a connection_id.
fn signaling_connect(agent: &ureq::Agent) -> Result<String, String> {
    let resp = post_json(
        agent,
        &format!("{SIGNALING_URL}/connect"),
        &serde_json::json!({ "device_id": DEVICE_ID }),
    )?;

    if let Some(id) = resp.get("connection_id").and_then(|v| v.as_str()) {
        Ok(id.to_string())
    } else if let Some(id) = resp.get("connection_id").and_then(|v| v.as_i64()) {
        Ok(id.to_string())
    } else {
        Err(format!("unexpected connect response: {resp}"))
    }
}

/// Send a message to the device via forward. Returns any device messages.
fn signaling_forward(
    agent: &ureq::Agent,
    conn_id: &str,
    msg: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, String> {
    let resp = post_json(
        agent,
        &format!("{SIGNALING_URL}/forward"),
        &serde_json::json!({
            "connection_id": conn_id,
            "payload": msg,
        }),
    )?;
    Ok(as_message_array(&resp))
}

/// Poll for messages from the device (POST, not GET).
fn signaling_poll(agent: &ureq::Agent, conn_id: &str) -> Result<Vec<serde_json::Value>, String> {
    let resp = post_json(
        agent,
        &format!("{SIGNALING_URL}/poll_messages"),
        &serde_json::json!({ "connection_id": conn_id }),
    )?;
    Ok(as_message_array(&resp))
}

/// Extract message array from response (may be array directly or {messages: [...]}).
fn as_message_array(resp: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = resp.as_array() {
        arr.clone()
    } else if let Some(arr) = resp.get("messages").and_then(|v| v.as_array()) {
        arr.clone()
    } else {
        vec![]
    }
}

// ── GStreamer pipeline ──

/// Shared state for the WebRTC session (used by ICE candidate callback).
struct SessionState {
    agent: ureq::Agent,
    conn_id: String,
}

/// Start the WebRTC display. Returns a GStreamer pipeline handle.
///
/// Verifies signaling server connectivity synchronously before building the
/// pipeline. Returns Err if the server is unreachable.
pub fn start_webrtc_display(
    picture: &gtk::Picture,
    running: Arc<AtomicBool>,
) -> Result<gst::Pipeline, String> {
    // Verify signaling server is reachable BEFORE building the pipeline.
    let http = make_agent();
    let conn_id = signaling_connect(&http)?;
    log::info!("webrtc: signaling connected");

    gst::init().map_err(|e| format!("GStreamer init: {e}"))?;

    let pipeline = gst::Pipeline::new();
    let webrtcbin = gst::ElementFactory::make("webrtcbin")
        .name("webrtc")
        .property_from_str("bundle-policy", "max-bundle")
        .build()
        .map_err(|e| format!("webrtcbin: {e}"))?;

    pipeline
        .add(&webrtcbin)
        .map_err(|e| format!("add webrtcbin: {e}"))?;

    // Video sink — use appsink to get decoded frames, render via GdkMemoryTexture
    // on the GTK main thread (avoids gtk4paintablesink GL threading crash on NVIDIA)
    let appsink = gst::ElementFactory::make("appsink")
        .name("videosink")
        .build()
        .map_err(|e| format!("appsink: {e}"))?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .build();
    appsink.set_property("caps", &caps);
    appsink.set_property("drop", true);
    appsink.set_property("max-buffers", 2u32);
    appsink.set_property("sync", true);

    let frame_slot: Arc<Mutex<Option<WebRtcFrame>>> = Arc::new(Mutex::new(None));
    let frame_slot_writer = frame_slot.clone();

    let app_sink = appsink
        .dynamic_cast::<gst_app::AppSink>()
        .map_err(|_| "cast to AppSink failed")?;

    app_sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                let info =
                    gst_video::VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::Error)?;

                let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                let data = map.as_slice().to_vec();

                *frame_slot_writer.lock().unwrap() = Some(WebRtcFrame {
                    width: info.width(),
                    height: info.height(),
                    stride: info.stride()[0] as u32,
                    data,
                });

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    let sink = app_sink.upcast::<gst::Element>();

    let convert = gst::ElementFactory::make("videoconvert")
        .name("convert")
        .build()
        .map_err(|e| format!("videoconvert: {e}"))?;
    let queue = gst::ElementFactory::make("queue")
        .name("videoqueue")
        .build()
        .map_err(|e| format!("queue: {e}"))?;

    pipeline
        .add_many([&convert, &queue, &sink])
        .map_err(|e| format!("add elements: {e}"))?;
    gst::Element::link_many([&convert, &queue, &sink])
        .map_err(|e| format!("link convert→queue→sink: {e}"))?;

    start_frame_renderer(picture, frame_slot);

    // Dynamic pad linking — detect codec from RTP caps and build decode chain
    let pipeline_weak = pipeline.downgrade();
    let convert_clone = convert.clone();
    webrtcbin.connect_pad_added(move |_webrtc, pad| {
        let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));

        let is_rtp = caps
            .structure(0)
            .map_or(false, |s| s.name().starts_with("application/x-rtp"));
        if !is_rtp {
            return;
        }

        let media_type = caps.structure(0).and_then(|s| s.get::<&str>("media").ok());
        if media_type != Some("video") {
            return;
        }

        let convert_sink = convert_clone.static_pad("sink").unwrap();
        if convert_sink.is_linked() {
            return;
        }

        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };

        let encoding = caps
            .structure(0)
            .and_then(|s| s.get::<&str>("encoding-name").ok())
            .unwrap_or("VP8");

        let (depay, _decoder) = match encoding {
            "H264" => {
                let nvdec = gst::ElementFactory::find("nvh264dec").is_some();
                let dec_name = if nvdec { "nvh264dec" } else { "avdec_h264" };
                log::info!("webrtc: H264 stream, decoder={dec_name}");

                let depay = gst::ElementFactory::make("rtph264depay").build().unwrap();
                let parse = gst::ElementFactory::make("h264parse").build().unwrap();
                let dec = gst::ElementFactory::make(dec_name).build().unwrap();

                pipeline.add_many([&depay, &parse, &dec]).unwrap();
                gst::Element::link_many([&depay, &parse, &dec]).unwrap();
                dec.link(&convert_clone).unwrap();

                for e in [&depay, &parse, &dec] {
                    let _ = e.sync_state_with_parent();
                }
                (depay, dec)
            }
            "VP9" => {
                log::info!("webrtc: VP9 stream, decoder=vp9dec");

                let depay = gst::ElementFactory::make("rtpvp9depay").build().unwrap();
                let dec = gst::ElementFactory::make("vp9dec").build().unwrap();

                pipeline.add_many([&depay, &dec]).unwrap();
                depay.link(&dec).unwrap();
                dec.link(&convert_clone).unwrap();

                for e in [&depay, &dec] {
                    let _ = e.sync_state_with_parent();
                }
                (depay, dec)
            }
            _ => {
                log::info!("webrtc: {encoding} stream, decoder=vp8dec");

                let depay = gst::ElementFactory::make("rtpvp8depay").build().unwrap();
                let dec = gst::ElementFactory::make("vp8dec").build().unwrap();

                pipeline.add_many([&depay, &dec]).unwrap();
                depay.link(&dec).unwrap();
                dec.link(&convert_clone).unwrap();

                for e in [&depay, &dec] {
                    let _ = e.sync_state_with_parent();
                }
                (depay, dec)
            }
        };

        let depay_sink = depay.static_pad("sink").unwrap();
        if let Err(e) = pad.link(&depay_sink) {
            log::error!("webrtc: pad link failed: {e}");
        } else {
            log::info!("webrtc: video pipeline linked ({encoding})");
        }
    });

    // ICE candidate handler — forward to signaling server
    let session: Arc<Mutex<Option<SessionState>>> = Arc::new(Mutex::new(None));
    let session_ice = session.clone();
    webrtcbin.connect("on-ice-candidate", false, move |values| {
        let mline_index = values[1].get::<u32>().unwrap();
        let candidate = values[2].get::<String>().unwrap();

        if candidate.is_empty() {
            return None;
        }

        if let Ok(guard) = session_ice.lock() {
            if let Some(sess) = guard.as_ref() {
                let msg = serde_json::json!({
                    "type": "ice-candidate",
                    "candidate": {
                        "sdpMid": mline_index.to_string(),
                        "sdpMLineIndex": mline_index,
                        "candidate": candidate,
                    },
                });
                if let Err(e) = signaling_forward(&sess.agent, &sess.conn_id, &msg) {
                    log::error!("webrtc: ICE forward failed: {e}");
                }
            }
        }
        None
    });

    // Monitor connection state
    webrtcbin.connect_notify(Some("ice-connection-state"), |webrtc, _| {
        let state = webrtc.property::<gst_webrtc::WebRTCICEConnectionState>("ice-connection-state");
        log::info!("webrtc: ICE state: {state:?}");
    });

    webrtcbin.connect("on-data-channel", false, move |values| {
        let channel = values[1]
            .get::<gst::glib::Object>()
            .expect("data channel object");
        let label = channel.property::<String>("label");
        log::info!("webrtc: data channel opened: {label}");
        None
    });

    // Start pipeline
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|e| format!("pipeline start: {e}"))?;

    // Signaling thread
    let webrtc_clone = webrtcbin.clone();
    let pipeline_clone = pipeline.clone();

    std::thread::spawn(move || {
        if let Err(e) = run_signaling(
            webrtc_clone,
            pipeline_clone,
            session,
            running,
            http,
            conn_id,
        ) {
            log::error!("webrtc: signaling failed: {e}");
        }
    });

    Ok(pipeline)
}

/// Process device messages (offer, ICE candidates).
fn handle_device_messages(
    messages: &[serde_json::Value],
    webrtcbin: &gst::Element,
    session: &Arc<Mutex<Option<SessionState>>>,
    got_offer: &mut bool,
) -> Result<(), String> {
    for msg in messages {
        let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "offer" => {
                let sdp_str = msg
                    .get("sdp")
                    .and_then(|v| v.as_str())
                    .ok_or("no sdp in offer")?;

                log::info!("webrtc: received SDP offer ({} bytes)", sdp_str.len());

                let sdp = gst_sdp::SDPMessage::parse_buffer(sdp_str.as_bytes())
                    .map_err(|e| format!("parse offer SDP: {e}"))?;
                let offer = gst_webrtc::WebRTCSessionDescription::new(
                    gst_webrtc::WebRTCSDPType::Offer,
                    sdp,
                );

                webrtcbin
                    .emit_by_name::<()>("set-remote-description", &[&offer, &None::<gst::Promise>]);

                let (answer_tx, answer_rx) = std::sync::mpsc::channel();
                let promise = gst::Promise::with_change_func(move |reply| {
                    let sdp = reply.ok().flatten().and_then(|s| {
                        s.value("answer")
                            .ok()
                            .and_then(|v| v.get::<gst_webrtc::WebRTCSessionDescription>().ok())
                    });
                    let _ = answer_tx.send(sdp);
                });

                webrtcbin.emit_by_name::<()>("create-answer", &[&None::<gst::Structure>, &promise]);

                let answer_sdp = answer_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .map_err(|_| "timeout creating answer".to_string())?
                    .ok_or("failed to create answer")?;

                webrtcbin.emit_by_name::<()>(
                    "set-local-description",
                    &[&answer_sdp, &None::<gst::Promise>],
                );

                let answer_text = answer_sdp.sdp().to_string();
                if let Ok(guard) = session.lock() {
                    if let Some(sess) = guard.as_ref() {
                        let answer_msg = serde_json::json!({
                            "type": "answer",
                            "sdp": answer_text,
                        });
                        signaling_forward(&sess.agent, &sess.conn_id, &answer_msg)?;
                        log::info!("webrtc: SDP answer sent ({} bytes)", answer_text.len());
                    }
                }

                *got_offer = true;
            }
            "ice-candidate" => {
                let candidate = msg.get("candidate").and_then(|v| v.as_str()).unwrap_or("");
                let sdp_mline_index =
                    msg.get("mLineIndex").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                webrtcbin.emit_by_name::<()>("add-ice-candidate", &[&sdp_mline_index, &candidate]);
            }
            "error" => {
                let error = msg
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                log::error!("webrtc: device error: {error}");
            }
            _ => {
                log::debug!("webrtc: unknown message type: {msg_type}");
            }
        }
    }
    Ok(())
}

/// Signaling thread: request offer from device, exchange SDP, poll for ICE.
fn run_signaling(
    webrtcbin: gst::Element,
    pipeline: gst::Pipeline,
    session: Arc<Mutex<Option<SessionState>>>,
    running: Arc<AtomicBool>,
    http: ureq::Agent,
    conn_id: String,
) -> Result<(), String> {
    *session.lock().unwrap() = Some(SessionState {
        agent: http.clone(),
        conn_id: conn_id.clone(),
    });

    log::info!("webrtc: requesting offer from device");
    let initial_messages = signaling_forward(
        &http,
        &conn_id,
        &serde_json::json!({
            "type": "request-offer",
            "ice_servers": [],
        }),
    )?;

    let mut got_offer = false;
    handle_device_messages(&initial_messages, &webrtcbin, &session, &mut got_offer)?;

    let mut poll_count = 0u64;
    while running.load(Ordering::Relaxed) {
        let messages = signaling_poll(&http, &conn_id).unwrap_or_default();
        handle_device_messages(&messages, &webrtcbin, &session, &mut got_offer)?;

        poll_count += 1;
        if !got_offer && poll_count > 60 {
            return Err("timed out waiting for SDP offer from device".into());
        }

        if got_offer {
            std::thread::sleep(std::time::Duration::from_millis(200));
        } else {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    let _ = pipeline.set_state(gst::State::Null);
    Ok(())
}

/// GTK timer that polls decoded frames from appsink and renders via GdkMemoryTexture.
fn start_frame_renderer(picture: &gtk::Picture, frame_slot: Arc<Mutex<Option<WebRtcFrame>>>) {
    let pic = picture.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(8), move || {
        let frame = frame_slot.lock().unwrap().take();
        if let Some(f) = frame {
            let bytes = glib::Bytes::from_owned(f.data);
            let texture = gtk::gdk::MemoryTexture::new(
                f.width as i32,
                f.height as i32,
                gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
                &bytes,
                f.stride as usize,
            );
            pic.set_paintable(Some(&texture));
        }
        glib::ControlFlow::Continue
    });
}

/// Stop the WebRTC pipeline.
pub fn stop_pipeline(pipeline: &gst::Pipeline) {
    let _ = pipeline.set_state(gst::State::Null);
}
