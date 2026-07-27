use kursal_core::{
    api::{AppEvent, ConnectionStatus},
    apiserver::CoreEventEmitter,
    dto::{ContactResponse, MessageResponse},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, broadcast, oneshot::Sender};

static ONLINE_CONTACTS: LazyLock<std::sync::Mutex<HashSet<String>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));

async fn stamp_last_seen(handle: &AppHandle, contact_id_hex: &str) {
    use kursal_core::api::state::AppState;
    use tauri::Manager;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0);
    let state = handle.state::<AppState>();
    let db = state.db().await;
    let _ = kursal_core::storage::set_contact_last_seen(&db, contact_id_hex, now_ms);
}

fn emitter<S: serde::Serialize + Clone>(
    handle: &AppHandle,
    api_handle: &broadcast::Sender<CoreEventEmitter>,
    event: &str,
    payload: S,
) {
    api_handle
        .send(CoreEventEmitter {
            event: event.to_string(),
            payload: serde_json::to_value(payload.clone()).unwrap_or(serde_json::Value::Null),
        })
        .ok();

    handle.emit(event, payload).ok();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn dnd_active(spec: &str) -> bool {
    let parts: Vec<&str> = spec.split('|').collect();
    if parts.len() != 3 || parts[0] != "1" {
        return false;
    }
    let parse = |s: &str| -> Option<i32> {
        let mut it = s.split(':');
        let h: i32 = it.next()?.parse().ok()?;
        let m: i32 = it.next()?.parse().ok()?;
        Some(h * 60 + m)
    };
    let (start, end) = match (parse(parts[1]), parse(parts[2])) {
        (Some(s), Some(e)) => (s, e),
        _ => return false,
    };
    if start == end {
        return false;
    }
    use chrono::{Local, Timelike};
    let now = Local::now();
    let cur = now.hour() as i32 * 60 + now.minute() as i32;
    if start < end {
        cur >= start && cur < end
    } else {
        cur >= start || cur < end
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn maybe_background_notify(handle: &AppHandle, contact_id: &str, content: &str) {
    use kursal_core::api::state::AppState;
    use kursal_core::contacts::Contact;
    use kursal_core::identity::UserId;
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;

    let window_visible = handle
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if window_visible {
        return;
    }

    {
        use std::sync::atomic::Ordering;
        if let Some(bg) = handle.try_state::<crate::background::BackgroundState>() {
            bg.unread.fetch_add(1, Ordering::Relaxed);
        }
        crate::background::refresh_tray(handle);
    }

    let state = handle.state::<AppState>();
    let preview = {
        let db = state.db().await;
        kursal_core::storage::get_notification_preview(&db)
    };
    if preview == "none" {
        return;
    }

    let dnd = {
        let db = state.db().await;
        kursal_core::storage::get_notification_dnd(&db)
    };
    if dnd_active(&dnd) {
        return;
    }

    let muted = {
        let db = state.db().await;
        kursal_core::storage::get_contact_muted(&db, contact_id)
    };
    if muted {
        return;
    }

    let sender = {
        let bytes = match hex::decode(contact_id) {
            Ok(b) => b,
            Err(_) => return,
        };
        let arr: [u8; 32] = match bytes.try_into() {
            Ok(a) => a,
            Err(_) => return,
        };
        let db = state.db().await;
        Contact::load(&db, &UserId(arr))
            .ok()
            .flatten()
            .map(|c| c.display_name)
            .unwrap_or_else(|| "Kursal".to_string())
    };

    if let Some(bg) = handle.try_state::<crate::background::BackgroundState>() {
        *bg.pending_signal.lock().unwrap() = Some(crate::background::PendingSignal::OpenChat(
            contact_id.to_string(),
        ));
    }

    let builder = match preview.as_str() {
        "generic" => handle
            .notification()
            .builder()
            .title("Kursal")
            .body("New message"),
        "sender" => handle.notification().builder().title(sender),
        _ => handle.notification().builder().title(sender).body(content),
    };
    let _ = builder.show();
}

pub async fn handle_core_event(
    event: AppEvent,
    handle: &AppHandle,
    api_handle: &broadcast::Sender<CoreEventEmitter>,
    pending_nearby_clone: &Arc<Mutex<HashMap<String, Sender<bool>>>>,
) {
    match event {
        AppEvent::BackendSignal { signal, payload } => {
            emitter(
                handle,
                api_handle,
                "backend_signal",
                serde_json::json!({
                    "signal": signal,
                    "payload": payload
                }),
            );
        }

        AppEvent::MessageReceived {
            message,
            via_offline,
            ..
        } => {
            let mut resp = MessageResponse::from(message);
            resp.via_offline = via_offline;

            if !resp.via_offline {
                stamp_last_seen(handle, &resp.contact_id).await;
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let notify_args = (resp.contact_id.clone(), resp.content.clone());

            emitter(handle, api_handle, "message_received", resp);

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            maybe_background_notify(handle, &notify_args.0, &notify_args.1).await;
        }

        AppEvent::TypingIndicator { contact_id } => {
            emitter(
                handle,
                api_handle,
                "typing_indicator",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                }),
            );
        }

        AppEvent::MessagePinned {
            contact_id,
            message_id,
            pinned,
        } => {
            emitter(
                handle,
                api_handle,
                "message_pinned",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "pinned": pinned
                }),
            );
        }

        AppEvent::MessageEdited {
            contact_id,
            message_id,
            new_content,
        } => {
            emitter(
                handle,
                api_handle,
                "message_edited",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "newContent": new_content
                }),
            );
        }

        AppEvent::MessageDeleted {
            contact_id,
            message_id,
        } => {
            emitter(
                handle,
                api_handle,
                "message_deleted",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                }),
            );
        }

        AppEvent::ReactionAdded {
            contact_id,
            message_id,
            emoji,
        } => {
            emitter(
                handle,
                api_handle,
                "reaction_added",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "emoji": emoji,
                }),
            );
        }

        AppEvent::ReactionRemoved {
            contact_id,
            message_id,
            emoji,
        } => {
            emitter(
                handle,
                api_handle,
                "reaction_removed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "emoji": emoji,
                }),
            );
        }

        AppEvent::DeliveryConfirmed {
            message_id,
            contact_id,
        } => {
            emitter(
                handle,
                api_handle,
                "delivery_confirmed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0)
                }),
            );
        }

        AppEvent::MessagesRead {
            contact_id,
            message_ids,
        } => {
            emitter(
                handle,
                api_handle,
                "messages_read",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageIds": message_ids
                        .into_iter()
                        .map(|m| hex::encode(m.0))
                        .collect::<Vec<_>>(),
                }),
            );
        }

        AppEvent::MessageQueuedOffline {
            message_id,
            contact_id,
        } => {
            emitter(
                handle,
                api_handle,
                "message_queued_offline",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0)
                }),
            );
        }

        AppEvent::ContactAdded { contact } => {
            emitter(
                handle,
                api_handle,
                "contact_added",
                ContactResponse::from(contact),
            );
        }

        AppEvent::ContactUpdated { contact } => {
            emitter(
                handle,
                api_handle,
                "contact_updated",
                ContactResponse::from(contact),
            );
        }

        AppEvent::ConnectionChange { contact_id, status } => {
            let key = hex::encode(contact_id.0);
            let stamp = {
                let mut online = ONLINE_CONTACTS.lock().unwrap();
                match status {
                    ConnectionStatus::Direct
                    | ConnectionStatus::HolePunch
                    | ConnectionStatus::Relay => {
                        online.insert(key.clone());
                        true
                    }
                    ConnectionStatus::Disconnected => online.remove(&key),
                    ConnectionStatus::Connecting => false,
                }
            };
            if stamp {
                stamp_last_seen(handle, &key).await;
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use tauri::Manager;

                if let Some(bg) = handle.try_state::<crate::background::BackgroundState>() {
                    let key = hex::encode(contact_id.0);
                    {
                        let mut map = bg.conn_status.lock().unwrap();
                        match &status {
                            ConnectionStatus::Direct | ConnectionStatus::HolePunch => {
                                map.insert(key, crate::background::TrayLink::Direct);
                            }
                            ConnectionStatus::Relay => {
                                map.insert(key, crate::background::TrayLink::Relay);
                            }
                            ConnectionStatus::Connecting | ConnectionStatus::Disconnected => {
                                map.remove(&key);
                            }
                        }
                    }
                    crate::background::refresh_tray(handle);
                }
            }

            emitter(
                handle,
                api_handle,
                "connection_changed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "status": match status {
                        ConnectionStatus::Connecting => "connecting",
                        ConnectionStatus::Relay => "relay",
                        ConnectionStatus::HolePunch => "holepunch",
                        ConnectionStatus::Direct => "direct",
                        ConnectionStatus::Disconnected => "disconnected",
                    }
                }),
            );
        }

        AppEvent::NearbyRequest {
            peer_id,
            session_name,
            decision_tx,
        } => {
            pending_nearby_clone
                .lock()
                .await
                .insert(peer_id.clone(), decision_tx);

            emitter(
                handle,
                api_handle,
                "nearby_request",
                serde_json::json!({
                    "peerId": peer_id,
                    "sessionName": session_name
                }),
            );
        }

        AppEvent::ContactRemoved { contact_id } => {
            emitter(
                handle,
                api_handle,
                "contact_removed",
                serde_json::json!({
                    "peerId": hex::encode(contact_id.0),
                }),
            );
        }
        AppEvent::ContactTerminated {
            contact_id,
            terminated,
        } => {
            emitter(
                handle,
                api_handle,
                "contact_terminated",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "terminated": terminated,
                }),
            );
        }
        AppEvent::FileOffered {
            contact_id,
            offer_id,
            filename,
            size_bytes,
            autodownload,
        } => {
            emitter(
                handle,
                api_handle,
                "file_offered",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "offerId": hex::encode(offer_id.0),
                    "filename": filename,
                    "sizeBytes": size_bytes,
                    "autodownload": autodownload,
                }),
            );
        }
        AppEvent::FileTransferProgress {
            transfer_id,
            bytes_transferred,
            total_bytes,
        } => {
            emitter(
                handle,
                api_handle,
                "file_transfer_progress",
                serde_json::json!({
                    "transferId": hex::encode(transfer_id.0),
                    "bytesTransferred": bytes_transferred,
                    "totalBytes": total_bytes
                }),
            );
        }
        AppEvent::FileReceived {
            contact_id,
            transfer_id,
            save_path,
        } => {
            emitter(
                handle,
                api_handle,
                "file_received",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "transferId": hex::encode(transfer_id.0),
                    "savePath": save_path,
                }),
            );
        }
        AppEvent::FileTransferFailed {
            contact_id,
            transfer_id,
            reason,
        } => {
            emitter(
                handle,
                api_handle,
                "file_transfer_failed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "transferId": hex::encode(transfer_id.0),
                    "reason": reason,
                }),
            );
        }
        AppEvent::NetworkOnline { online, peer_count } => {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use std::sync::atomic::Ordering;
                use tauri::Manager;

                if let Some(bg) = handle.try_state::<crate::background::BackgroundState>() {
                    bg.net_online.store(online, Ordering::Relaxed);
                    bg.net_peers.store(peer_count, Ordering::Relaxed);
                }
                crate::background::refresh_tray(handle);
            }

            emitter(
                handle,
                api_handle,
                "network_online",
                serde_json::json!({
                    "online": online,
                    "peerCount": peer_count,
                }),
            );
        }
        AppEvent::OfflineBundlePublished {
            contact_id,
            message_ids,
        } => {
            emitter(
                handle,
                api_handle,
                "offline_bundle_published",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageIds": message_ids
                        .into_iter()
                        .map(|m| hex::encode(m.0))
                        .collect::<Vec<_>>(),
                }),
            );
        }

        AppEvent::MessageFailed {
            contact_id,
            message_ids,
        } => {
            emitter(
                handle,
                api_handle,
                "message_failed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageIds": message_ids
                        .into_iter()
                        .map(|m| hex::encode(m.0))
                        .collect::<Vec<_>>(),
                }),
            );
        }

        AppEvent::OfflineGapSkipped {
            contact_id,
            counter,
        } => {
            emitter(
                handle,
                api_handle,
                "offline_gap_skipped",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "counter": counter,
                }),
            );
        }

        AppEvent::OfflineQueueDrained { contact_id } => {
            emitter(
                handle,
                api_handle,
                "offline_queue_drained",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                }),
            );
        }

        AppEvent::OfflineSync { active } => {
            emitter(
                handle,
                api_handle,
                "offline_sync",
                serde_json::json!({
                    "active": active,
                }),
            );
        }

        AppEvent::CallIncoming {
            call_id,
            contact_id,
            sample_rate,
        } => {
            emitter(
                handle,
                api_handle,
                "call_incoming",
                serde_json::json!({
                    "callId": hex::encode(call_id.0),
                    "contactId": hex::encode(contact_id.0),
                    "sampleRate": sample_rate
                }),
            );
        }

        AppEvent::CallState { call_id, state } => {
            emitter(
                handle,
                api_handle,
                "call_state",
                serde_json::json!({
                    "callId": hex::encode(call_id.0),
                    "state": state
                }),
            );
        }

        AppEvent::CallLevel { mic, remote } => {
            emitter(
                handle,
                api_handle,
                "call_level",
                serde_json::json!({
                    "mic": mic,
                    "remote": remote
                }),
            );
        }

        AppEvent::CallEnded {
            call_id,
            reason,
            duration_ms,
        } => {
            emitter(
                handle,
                api_handle,
                "call_ended",
                serde_json::json!({
                    "callId": hex::encode(call_id.0),
                    "reason": reason,
                    "durationMs": duration_ms
                }),
            );
        }

        AppEvent::VideoState {
            call_id,
            active,
            codec,
            width,
            height,
            reason,
        } => {
            emitter(
                handle,
                api_handle,
                "video_state",
                serde_json::json!({
                    "callId": hex::encode(call_id.0),
                    "active": active,
                    "codec": codec,
                    "width": width,
                    "height": height,
                    "reason": reason
                }),
            );
        }

        AppEvent::VideoKeyframeRequested { call_id } => {
            emitter(
                handle,
                api_handle,
                "video_keyframe_requested",
                serde_json::json!({
                    "callId": hex::encode(call_id.0)
                }),
            );
        }

        AppEvent::CallPeerVoiceState {
            call_id,
            muted,
            deafened,
        } => {
            emitter(
                handle,
                api_handle,
                "call_peer_voice_state",
                serde_json::json!({
                    "callId": hex::encode(call_id.0),
                    "muted": muted,
                    "deafened": deafened
                }),
            );
        }

        AppEvent::VideoCongestion => {
            emitter(
                handle,
                api_handle,
                "video_congestion",
                serde_json::json!({}),
            );
        }
    }
}
