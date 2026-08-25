use crate::MapKursalResult;
use crate::api::{AppEvent, send_message};
use crate::call::signal::{
    self, AnswerPayload, HangupPayload, HangupReason, OfferPayload, VideoStartPayload,
    VideoStopPayload, VideoStopReason, VoiceStatePayload,
};
use crate::call::{CallAction, CallEngine, CallInput, CallState};
use crate::contacts::Contact;
use crate::identity::UserId;
use crate::messaging::StoredMessage;
use crate::messaging::enums::{
    CallOutcome, CallRecordMessage, CallSignal, CallSignalKind, Direction, KursalMessage,
    MessageId, MessageStatus,
};
use crate::network::swarm::SwarmCommand;
use crate::storage::{SharedDatabase, get_timestamp_secs};
use crate::{KursalError, Result};
use std::sync::OnceLock;
use tokio::sync::{Mutex, mpsc};

pub const CALL_SAMPLE_RATE: u32 = 48000;
pub const CALL_CHANNELS: u8 = 1;

pub fn build_signal(
    call_id: MessageId,
    action: &CallAction,
    sample_rate: u32,
) -> Result<Option<KursalMessage>> {
    let cs = match action {
        CallAction::SendOffer { random } => CallSignal {
            call_id,
            kind: CallSignalKind::Offer,
            payload: signal::encode(&OfferPayload {
                random: *random,
                sample_rate,
                channels: CALL_CHANNELS,
            })?,
        },
        CallAction::SendAnswer { random } => CallSignal {
            call_id,
            kind: CallSignalKind::Answer,
            payload: signal::encode(&AnswerPayload { random: *random })?,
        },
        CallAction::SendHangup { reason } => CallSignal {
            call_id,
            kind: CallSignalKind::Hangup,
            payload: signal::encode(&HangupPayload { reason: *reason })?,
        },
        _ => return Ok(None),
    };
    Ok(Some(KursalMessage::CallSignal(cs)))
}

pub fn build_video_start(
    call_id: MessageId,
    codec: String,
    width: u16,
    height: u16,
) -> Result<KursalMessage> {
    Ok(KursalMessage::CallSignal(CallSignal {
        call_id,
        kind: CallSignalKind::VideoStart,
        payload: signal::encode(&VideoStartPayload {
            codec,
            width,
            height,
        })?,
    }))
}

pub fn build_video_stop(call_id: MessageId, reason: VideoStopReason) -> Result<KursalMessage> {
    Ok(KursalMessage::CallSignal(CallSignal {
        call_id,
        kind: CallSignalKind::VideoStop,
        payload: signal::encode(&VideoStopPayload { reason })?,
    }))
}

pub fn build_video_keyframe_request(call_id: MessageId) -> Result<KursalMessage> {
    Ok(KursalMessage::CallSignal(CallSignal {
        call_id,
        kind: CallSignalKind::VideoKeyframeRequest,
        payload: Vec::new(),
    }))
}

pub fn build_voice_state(call_id: MessageId, muted: bool, deafened: bool) -> Result<KursalMessage> {
    Ok(KursalMessage::CallSignal(CallSignal {
        call_id,
        kind: CallSignalKind::VoiceState,
        payload: signal::encode(&VoiceStatePayload { muted, deafened })?,
    }))
}

async fn broadcast_voice_state(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let Some((muted, deafened)) = crate::call::media::voice_state().await else {
        return Ok(());
    };
    let Ok((call_id, peer, ..)) = connected_call_context().await else {
        return Ok(());
    };
    let msg = build_voice_state(call_id, muted, deafened)?;
    send_message(msg, &peer, db, cmd_tx, Some(app_event_tx)).await?;
    Ok(())
}

fn parse_stop_reason(reason: &str) -> VideoStopReason {
    match reason {
        "unsupported" => VideoStopReason::Unsupported,
        "error" => VideoStopReason::Error,
        _ => VideoStopReason::Toggle,
    }
}

fn stop_reason_str(reason: VideoStopReason) -> &'static str {
    match reason {
        VideoStopReason::Toggle => "toggle",
        VideoStopReason::Unsupported => "unsupported",
        VideoStopReason::Error => "error",
    }
}

async fn connected_call_context()
-> Result<(MessageId, Contact, libp2p::PeerId, [u8; 32], [u8; 32], bool)> {
    let guard = slot().lock().await;
    let active = guard
        .as_ref()
        .filter(|a| matches!(a.engine.state, CallState::Connected))
        .ok_or_else(|| KursalError::Network("no connected call".into()))?;
    let (Some(my), Some(their)) = (active.engine.my_random, active.engine.their_random) else {
        return Err(KursalError::Network("call keys unavailable".into()));
    };
    let peer_id = active
        .peer
        .peer_id
        .parse::<libp2p::PeerId>()
        .map_err(|_| KursalError::Network("invalid peer id".into()))?;
    Ok((
        active.call_id,
        active.peer.clone(),
        peer_id,
        my,
        their,
        active.engine.is_caller,
    ))
}

fn arm_capture_watchdog(
    db: SharedDatabase,
    cmd_tx: mpsc::Sender<SwarmCommand>,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    tokio::task::spawn_local(async move {
        if crate::call::capture::failed().await {
            log::error!("[call] video capture died, stopping video");
            let _ = stop_video("error".to_string(), db, &cmd_tx, &app_event_tx).await;
        }
    });
}

pub async fn start_video(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let (call_id, peer, peer_id, my, their, is_caller) = connected_call_context().await?;
    let keys = crate::call::crypto::derive_video_keys(my, their, is_caller)?;

    let quality = crate::storage::get_video_quality(&db.0);
    let camera_id = crate::call::capture::selected_camera();
    crate::call::capture::set_runtime(tokio::runtime::Handle::current());

    let format = tokio::task::spawn_blocking(move || {
        crate::call::capture::start(crate::call::capture::CaptureConfig { quality, camera_id })
    })
    .await
    .map_err(|e| KursalError::Misc(anyhow::anyhow!("video capture task: {e}")))??;

    crate::call::video::start_tx(peer_id, keys.tx, cmd_tx.clone(), app_event_tx.clone());
    arm_capture_watchdog(db.clone(), cmd_tx.clone(), app_event_tx.clone());

    let _ = app_event_tx
        .send(AppEvent::VideoLocalState {
            active: true,
            codec: Some(format.codec.clone()),
            width: format.width,
            height: format.height,
            camera_id: format.camera_id.clone(),
        })
        .await;

    crate::call::capture::request_keyframe();

    let msg = build_video_start(call_id, format.codec, format.width, format.height)?;
    send_message(msg, &peer, db, cmd_tx, Some(app_event_tx)).await?;
    Ok(())
}

pub async fn list_cameras() -> Vec<crate::dto::CameraInfo> {
    tokio::task::spawn_blocking(crate::call::capture::list_cameras)
        .await
        .unwrap_or_default()
}

pub async fn refresh_camera_rotation() {
    let _ = tokio::task::spawn_blocking(crate::call::capture::refresh_rotation).await;
}

pub async fn set_camera(
    camera_id: Option<String>,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let previous = crate::call::capture::selected_camera();
    crate::call::capture::set_selected_camera(camera_id);
    if !crate::call::capture::is_active() {
        return Ok(());
    }

    let Err(e) = start_video(db.clone(), cmd_tx, app_event_tx).await else {
        return Ok(());
    };

    log::warn!("[call] camera switch failed: {e}");
    crate::call::capture::set_selected_camera(previous);
    if start_video(db.clone(), cmd_tx, app_event_tx).await.is_err() {
        let _ = stop_video("error".to_string(), db, cmd_tx, app_event_tx).await;
    }
    Err(e)
}

pub async fn stop_video(
    reason: String,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let _ = tokio::task::spawn_blocking(crate::call::capture::stop).await;
    crate::call::video::clear_local_forwarder();
    crate::call::video::stop_tx().await;
    let _ = app_event_tx
        .send(AppEvent::VideoLocalState {
            active: false,
            codec: None,
            width: 0,
            height: 0,
            camera_id: None,
        })
        .await;
    let (call_id, peer, ..) = connected_call_context().await?;
    let msg = build_video_stop(call_id, parse_stop_reason(&reason))?;
    send_message(msg, &peer, db, cmd_tx, Some(app_event_tx)).await?;
    Ok(())
}

pub async fn request_video_keyframe(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let (call_id, peer, ..) = connected_call_context().await?;
    let msg = build_video_keyframe_request(call_id)?;
    send_message(msg, &peer, db, cmd_tx, Some(app_event_tx)).await?;
    Ok(())
}

pub fn state_str(state: CallState) -> &'static str {
    match state {
        CallState::Idle => "idle",
        CallState::RingingOut => "ringing_out",
        CallState::RingingIn => "ringing_in",
        CallState::Connecting => "connecting",
        CallState::Connected => "connected",
        CallState::Ended => "ended",
    }
}

struct ActiveCall {
    engine: CallEngine,
    peer: Contact,
    call_id: MessageId,
    started_at: Option<u64>,
    sample_rate: u32,
}

static ACTIVE: OnceLock<Mutex<Option<ActiveCall>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<ActiveCall>> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}

const RING_TIMEOUT_SECS: u64 = 45;

fn spawn_ring_timeout(
    call_id: MessageId,
    db: SharedDatabase,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(RING_TIMEOUT_SECS)).await;
        timeout_fire(call_id, db, app_event_tx).await;
    });
}

async fn timeout_fire(
    call_id: MessageId,
    db: SharedDatabase,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    let mut guard = slot().lock().await;
    let ringing = match guard.as_ref() {
        Some(a) => {
            a.call_id == call_id
                && matches!(a.engine.state, CallState::RingingOut | CallState::RingingIn)
        }
        None => false,
    };

    if !ringing {
        return;
    }

    let mut record: Option<(Contact, bool)> = None;
    if let Some(active) = guard.as_mut() {
        active.engine.step(CallInput::Timeout);
        let state = state_str(active.engine.state).to_string();

        app_event_tx
            .send(AppEvent::CallState { call_id, state })
            .await
            .ok();

        app_event_tx
            .send(AppEvent::CallEnded {
                call_id,
                reason: "timeout".to_string(),
                duration_ms: 0,
            })
            .await
            .ok();

        record = Some((active.peer.clone(), active.engine.is_caller));
    }

    crate::call::media::stop().await;
    crate::call::video::stop_all().await;
    *guard = None;
    drop(guard);

    if let Some((contact, is_caller)) = record {
        store_call_record(
            &db,
            &contact,
            is_caller,
            CallOutcome::Missed,
            0,
            &app_event_tx,
        )
        .await;
    }
}

async fn execute(
    actions: Vec<CallAction>,
    call_id: MessageId,
    peer: &Contact,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
    sample_rate: u32,
) -> Result<()> {
    for action in &actions {
        match action {
            CallAction::SendOffer { .. }
            | CallAction::SendAnswer { .. }
            | CallAction::SendHangup { .. } => {
                if let Some(msg) = build_signal(call_id, action, sample_rate)? {
                    send_message(msg, peer, db.clone(), cmd_tx, Some(app_event_tx)).await?;
                }
            }
            CallAction::Emit(state) => {
                app_event_tx
                    .send(AppEvent::CallState {
                        call_id,
                        state: state_str(*state).to_string(),
                    })
                    .await
                    .ok();
            }
            CallAction::OpenMedia | CallAction::StartMedia | CallAction::StopMedia => {}
        }
    }
    Ok(())
}

fn outcome_for(reason: &str, duration_ms: u64, is_caller: bool) -> Option<CallOutcome> {
    if duration_ms > 0 {
        return Some(CallOutcome::Completed);
    }
    match reason {
        "timeout" => Some(CallOutcome::Missed),
        "declined" => Some(CallOutcome::Declined),
        "busy" => Some(CallOutcome::Busy),
        "hangup" | "ended" => Some(if is_caller {
            CallOutcome::Canceled
        } else {
            CallOutcome::Missed
        }),
        _ => None,
    }
}

async fn store_call_record(
    db: &SharedDatabase,
    contact: &Contact,
    is_caller: bool,
    outcome: CallOutcome,
    duration_ms: u64,
    app_event_tx: &mpsc::Sender<AppEvent>,
) {
    let now = match get_timestamp_secs() {
        Ok(t) => t,
        Err(_) => return,
    };
    let id = MessageId::new();

    let stored = StoredMessage {
        id,
        contact_id: contact.user_id.clone(),
        payload: KursalMessage::CallRecord(CallRecordMessage {
            id,
            timestamp: now,
            outcome,
            duration_ms,
        }),
        timestamp: now,
        direction: if is_caller {
            Direction::Sent
        } else {
            Direction::Received
        },
        status: MessageStatus::Delivered,
        raw_ciphertext: None,
        edited: false,
        pinned: false,
        reactions: Vec::new(),
    };

    if stored.save(db).is_err() {
        return;
    }

    let _ = app_event_tx
        .send(AppEvent::MessageReceived {
            contact_id: contact.user_id.clone(),
            message: stored,
            via_offline: false,
        })
        .await;
}

fn end_reason(input: &CallInput) -> &'static str {
    match input {
        CallInput::Decline => "declined",
        CallInput::LocalHangup => "hangup",
        CallInput::Timeout => "timeout",
        CallInput::HangupRecv { reason } => match reason {
            HangupReason::Declined => "declined",
            HangupReason::Busy => "busy",
            HangupReason::Timeout => "timeout",
            HangupReason::Hangup | HangupReason::Error => "hangup",
        },
        _ => "ended",
    }
}

async fn run_step(
    active: &mut ActiveCall,
    input: CallInput,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<bool> {
    let reason = end_reason(&input);
    let was_connected = matches!(active.engine.state, CallState::Connected);
    let mut actions = active.engine.step(input);
    if matches!(active.engine.state, CallState::Connecting) {
        actions.extend(active.engine.step(CallInput::MediaUp));
    }

    if matches!(active.engine.state, CallState::Connected) && active.started_at.is_none() {
        active.started_at = Some(get_timestamp_secs()?);
    }

    if !was_connected && matches!(active.engine.state, CallState::Connected) {
        store_call_record(
            &db,
            &active.peer,
            active.engine.is_caller,
            CallOutcome::Started,
            0,
            app_event_tx,
        )
        .await;
    }

    if !was_connected
        && matches!(active.engine.state, CallState::Connected)
        && let (Some(Ok(keys)), Ok(peer_id)) = (
            active.engine.derive_keys(),
            active.peer.peer_id.parse::<libp2p::PeerId>(),
        )
    {
        crate::call::media::establish(
            active.engine.is_caller,
            peer_id,
            keys,
            cmd_tx.clone(),
            app_event_tx.clone(),
            active.sample_rate,
            active.call_id,
            db.clone(),
        );
    }

    let call_id = active.call_id;
    let peer = active.peer.clone();
    let sample_rate = active.sample_rate;
    execute(
        actions,
        call_id,
        &peer,
        db.clone(),
        cmd_tx,
        app_event_tx,
        sample_rate,
    )
    .await?;

    if matches!(active.engine.state, CallState::Ended) {
        crate::call::media::stop().await;
        crate::call::video::stop_all().await;
        let duration_ms = match active.started_at {
            Some(s) => get_timestamp_secs()?.saturating_sub(s).saturating_mul(1000),
            None => 0,
        };
        app_event_tx
            .send(AppEvent::CallEnded {
                call_id,
                reason: reason.to_string(),
                duration_ms,
            })
            .await
            .ok();

        if let Some(outcome) = outcome_for(reason, duration_ms, active.engine.is_caller) {
            store_call_record(
                &db,
                &active.peer,
                active.engine.is_caller,
                outcome,
                duration_ms,
                app_event_tx,
            )
            .await;
        }
        return Ok(true);
    }
    Ok(false)
}

pub async fn start(
    contact_id: &str,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<MessageId> {
    let bytes: [u8; 32] = hex::decode(contact_id)
        .ok_kursal(KursalError::Crypto)?
        .try_into()
        .map_err(|_| KursalError::Crypto("invalid contact id length".into()))?;

    let peer = Contact::load(&db, &UserId(bytes))?
        .ok_or_else(|| KursalError::Storage("contact not found".into()))?;

    let sample_rate = crate::storage::get_call_sample_rate(&db);

    let mut guard = slot().lock().await;
    if guard.is_some() {
        return Err(KursalError::Network("already in a call".into()));
    }

    let mut active = ActiveCall {
        engine: CallEngine::new(),
        peer,
        call_id: MessageId::new(),
        started_at: None,
        sample_rate,
    };

    let call_id = active.call_id;
    let ended = run_step(
        &mut active,
        CallInput::StartCall,
        db.clone(),
        cmd_tx,
        app_event_tx,
    )
    .await?;

    if !ended {
        *guard = Some(active);
        spawn_ring_timeout(call_id, db, app_event_tx.clone());
    }
    Ok(call_id)
}

pub async fn accept(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let mut guard = slot().lock().await;
    let ended = match guard.as_mut() {
        Some(active) => run_step(active, CallInput::Accept, db, cmd_tx, app_event_tx).await?,
        None => return Ok(()),
    };

    if ended {
        *guard = None;
    }

    Ok(())
}

pub async fn decline(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let mut guard = slot().lock().await;
    let ended = match guard.as_mut() {
        Some(active) => run_step(active, CallInput::Decline, db, cmd_tx, app_event_tx).await?,
        None => return Ok(()),
    };

    if ended {
        *guard = None;
    }

    Ok(())
}

pub async fn hangup(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let mut guard = slot().lock().await;
    let ended = match guard.as_mut() {
        Some(active) => run_step(active, CallInput::LocalHangup, db, cmd_tx, app_event_tx).await?,
        None => return Ok(()),
    };

    if ended {
        *guard = None;
    }

    Ok(())
}

pub async fn media_failed(
    call_id: MessageId,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let mut guard = slot().lock().await;
    let ended = match guard.as_mut() {
        Some(active) if active.call_id == call_id => {
            run_step(active, CallInput::LocalHangup, db, cmd_tx, app_event_tx).await?
        }
        _ => return Ok(()),
    };

    if ended {
        *guard = None;
    }

    Ok(())
}

pub async fn set_mute(
    muted: bool,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    crate::call::media::set_muted(muted).await;
    let _ = broadcast_voice_state(db, cmd_tx, app_event_tx).await;
    Ok(())
}

pub async fn set_deafen(
    deafened: bool,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    crate::call::media::set_deafened(deafened).await;
    let _ = broadcast_voice_state(db, cmd_tx, app_event_tx).await;
    Ok(())
}

pub async fn set_audio_device(kind: String, name: Option<String>) -> Result<()> {
    match kind.as_str() {
        "input" => crate::call::audio::set_input_device(name),
        "output" => crate::call::audio::set_output_device(name),
        _ => {}
    }
    Ok(())
}

pub fn list_audio_devices() -> crate::call::audio::AudioDevices {
    crate::call::audio::list_devices()
}

pub async fn on_signal(
    from: &Contact,
    signal: CallSignal,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let mut guard = slot().lock().await;
    match signal.kind {
        CallSignalKind::Offer => {
            if guard.is_some() {
                if let Some(msg) = build_signal(
                    signal.call_id,
                    &CallAction::SendHangup {
                        reason: HangupReason::Busy,
                    },
                    CALL_SAMPLE_RATE,
                )? {
                    send_message(msg, from, db.clone(), cmd_tx, Some(app_event_tx)).await?;
                }
                return Ok(());
            }

            let payload: OfferPayload = signal::decode(&signal.payload)?;
            let sample_rate = crate::storage::valid_call_sample_rate(payload.sample_rate);
            let mut active = ActiveCall {
                engine: CallEngine::new(),
                peer: from.clone(),
                call_id: signal.call_id,
                started_at: None,
                sample_rate,
            };

            app_event_tx
                .send(AppEvent::CallIncoming {
                    call_id: signal.call_id,
                    contact_id: from.user_id.clone(),
                    sample_rate,
                })
                .await
                .ok();

            let ended = run_step(
                &mut active,
                CallInput::OfferRecv {
                    random: payload.random,
                },
                db.clone(),
                cmd_tx,
                app_event_tx,
            )
            .await?;

            if !ended {
                *guard = Some(active);
                spawn_ring_timeout(signal.call_id, db, app_event_tx.clone());
            }
        }
        CallSignalKind::Answer => {
            let ended = match guard.as_mut() {
                Some(active)
                    if active.call_id == signal.call_id && active.peer.user_id == from.user_id =>
                {
                    let payload: AnswerPayload = signal::decode(&signal.payload)?;
                    run_step(
                        active,
                        CallInput::AnswerRecv {
                            random: payload.random,
                        },
                        db,
                        cmd_tx,
                        app_event_tx,
                    )
                    .await?
                }
                _ => false,
            };
            if ended {
                *guard = None;
            }
        }
        CallSignalKind::Hangup => {
            let matches_active = guard
                .as_ref()
                .is_some_and(|a| a.call_id == signal.call_id && a.peer.user_id == from.user_id);
            if matches_active {
                if let Some(active) = guard.as_mut() {
                    let payload: HangupPayload = signal::decode(&signal.payload)?;
                    run_step(
                        active,
                        CallInput::HangupRecv {
                            reason: payload.reason,
                        },
                        db,
                        cmd_tx,
                        app_event_tx,
                    )
                    .await?;
                }
                *guard = None;
            }
        }
        CallSignalKind::VideoStart => {
            let matches_active = guard.as_ref().is_some_and(|a| {
                a.call_id == signal.call_id
                    && a.peer.user_id == from.user_id
                    && matches!(a.engine.state, CallState::Connected)
            });
            if matches_active && let Some(active) = guard.as_ref() {
                let payload: VideoStartPayload = signal::decode(&signal.payload)?;
                if let (Some(my), Some(their), Ok(peer_id)) = (
                    active.engine.my_random,
                    active.engine.their_random,
                    active.peer.peer_id.parse::<libp2p::PeerId>(),
                ) {
                    let keys =
                        crate::call::crypto::derive_video_keys(my, their, active.engine.is_caller)?;
                    crate::call::video::start_rx(peer_id, keys.rx);
                    app_event_tx
                        .send(AppEvent::VideoState {
                            call_id: signal.call_id,
                            active: true,
                            codec: Some(payload.codec),
                            width: payload.width,
                            height: payload.height,
                            reason: None,
                        })
                        .await
                        .ok();
                }
            }
        }
        CallSignalKind::VideoStop => {
            let matches_active = guard.as_ref().is_some_and(|a| {
                a.call_id == signal.call_id
                    && a.peer.user_id == from.user_id
                    && matches!(a.engine.state, CallState::Connected)
            });
            if matches_active {
                let payload: VideoStopPayload = signal::decode(&signal.payload)?;
                let reason = match payload.reason {
                    VideoStopReason::Unsupported => {
                        let _ = tokio::task::spawn_blocking(crate::call::capture::stop).await;
                        crate::call::video::stop_tx().await;
                        "send_rejected"
                    }
                    other => {
                        crate::call::video::stop_rx().await;
                        stop_reason_str(other)
                    }
                };
                app_event_tx
                    .send(AppEvent::VideoState {
                        call_id: signal.call_id,
                        active: false,
                        codec: None,
                        width: 0,
                        height: 0,
                        reason: Some(reason.to_string()),
                    })
                    .await
                    .ok();
            }
        }
        CallSignalKind::VideoKeyframeRequest => {
            let matches_active = guard.as_ref().is_some_and(|a| {
                a.call_id == signal.call_id
                    && a.peer.user_id == from.user_id
                    && matches!(a.engine.state, CallState::Connected)
            });
            if matches_active {
                crate::call::capture::request_keyframe();
                app_event_tx
                    .send(AppEvent::VideoKeyframeRequested {
                        call_id: signal.call_id,
                    })
                    .await
                    .ok();
            }
        }
        CallSignalKind::VoiceState => {
            let matches_active = guard
                .as_ref()
                .is_some_and(|a| a.call_id == signal.call_id && a.peer.user_id == from.user_id);
            if matches_active {
                let payload: VoiceStatePayload = signal::decode(&signal.payload)?;
                app_event_tx
                    .send(AppEvent::CallPeerVoiceState {
                        call_id: signal.call_id,
                        muted: payload.muted,
                        deafened: payload.deafened,
                    })
                    .await
                    .ok();
            }
        }
        CallSignalKind::IceCandidate => {}
    }
    Ok(())
}
