use crate::MapKursalResult;
use crate::first_contact::ltc::LtcState;
use crate::{
    KursalError,
    api::{
        AppEvent, CoreCommand,
        file_transfers::{
            FileIncomingEntry, FileReceiveEntry, FileTransferEntry, apply_cancel,
            remove_contact_transfers, stage_outgoing,
        },
        send_message, send_message_tracked, share_profile_with,
    },
    contacts::Contact,
    crypto::stream::derive_stream_key,
    first_contact::{
        ltc::LtcPayload,
        nearby::nearby_connect,
        otp::{fetch_otp, publish_otp},
    },
    identity::{TransportIdentity, UserId},
    messaging::{
        StoredMessage,
        enums::{
            FileAccept, FileCancel, FileOffer, KursalMessage, MessageDelete, MessageEdit,
            MessageId, MessagePin, MessageStatus, ReactionAdd, ReactionRemove, ReadReceipt,
            TextMessage,
        },
    },
    network::{
        NetworkManager,
        swarm::{FILE_CHUNK_SIZE, SwarmCommand, SwarmHandle},
    },
    storage::{
        SharedDatabase, TABLE_FILE_TRANSFERS,
        file::KursalFile,
        filetransfer::{available_space, hash_file},
        get_timestamp_secs,
    },
};
use rand::{TryRngCore, rngs::OsRng};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{Mutex, mpsc};

fn spawn_pointer_publish(
    db: SharedDatabase,
    swarm: SwarmHandle,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    tokio::task::spawn_local(async move {
        if let Err(err) = LtcState::publish_pointer(db, swarm, app_event_tx).await {
            log::warn!("[ltc] rendezvous publish failed: {err}");
        }
    });
}

pub async fn handle_core_command(
    cmd: CoreCommand,
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    match cmd {
        CoreCommand::PublishOtp { otp, reply } => {
            let swarm = network.lock().await.primary.clone();
            let result = publish_otp(&otp, db, &swarm).await;
            reply.send(result).ok();
        }

        CoreCommand::FetchOtp { otp, reply } => {
            let swarm = network.lock().await.primary.clone();
            let result = fetch_otp(&otp, db, &swarm).await;
            reply.send(result).ok();
        }

        CoreCommand::GetLtcStatus { reply } => {
            let result =
                LtcState::load(&db).map(|opt| opt.and_then(|p| LtcState::dto_serialize(&p).ok()));

            reply.send(result).ok();
        }

        CoreCommand::CreateLtc {
            max_uses,
            ttl_secs,
            reply,
        } => {
            let swarm = network.lock().await.primary.clone();
            let result = LtcState::create(db.clone(), &swarm.cmd_tx, max_uses, ttl_secs)
                .await
                .and_then(|p| LtcState::dto_serialize(&p));
            let status = result.as_ref().ok().cloned();

            app_event_tx
                .send(AppEvent::LtcUpdated { status })
                .await
                .ok();

            if result.is_ok() {
                spawn_pointer_publish(db, swarm, app_event_tx.clone());
            }

            reply.send(result).ok();
        }

        CoreCommand::UpdateLtcLimits {
            max_uses,
            ttl_secs,
            reply,
        } => {
            let result = LtcState::update_limits(db, max_uses, ttl_secs).await;
            let status = result.as_ref().ok().cloned();

            app_event_tx
                .send(AppEvent::LtcUpdated { status })
                .await
                .ok();

            reply.send(result).ok();
        }

        CoreCommand::ExportLtc { reply } => {
            let swarm = network.lock().await.primary.clone();
            let result = LtcState::export_ltc(db, &swarm).await;

            reply.send(result).ok();
        }

        CoreCommand::SetLtcFollowRotations { enabled, reply } => {
            let swarm = network.lock().await.primary.clone();
            let result = LtcState::set_follow_rotations(db.clone(), &swarm.cmd_tx, enabled).await;
            let status = result.as_ref().ok().cloned();

            app_event_tx
                .send(AppEvent::LtcUpdated { status })
                .await
                .ok();

            if enabled && result.is_ok() {
                spawn_pointer_publish(db, swarm, app_event_tx.clone());
            }

            reply.send(result).ok();
        }

        CoreCommand::RepublishLtcPointer { reply } => {
            let swarm = network.lock().await.primary.clone();
            let result = {
                match LtcState::load(&db) {
                    Ok(Some(state)) => state.dto_serialize(),
                    Ok(None) => Err(KursalError::Storage("No LTC currently stored".to_string())),
                    Err(err) => Err(err),
                }
            };

            if result.is_ok() {
                spawn_pointer_publish(db, swarm, app_event_tx.clone());
            }

            reply.send(result).ok();
        }

        CoreCommand::RevokeLtc { reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = LtcState::revoke_ltc(db, &cmd_tx).await;

            app_event_tx
                .send(AppEvent::LtcUpdated { status: None })
                .await
                .ok();

            reply.send(result).ok();
        }

        CoreCommand::ImportLtc { bytes, reply } => {
            let swarm = network.lock().await.primary.clone();

            let result = match KursalFile::deserialize(&bytes) {
                Ok(KursalFile::LtcPayload(bytes)) => LtcPayload::deserialize(&bytes),
                _ => Err(KursalError::Identity("Invalid file type".to_string())),
            };

            let result = match result {
                Ok(payload) => LtcState::import_ltc(payload, db, &swarm).await,
                Err(e) => Err(e),
            };

            reply.send(result).ok();
        }

        CoreCommand::ConnectNearby {
            peer_id,
            session_name,
            method,
            reply,
        } => {
            let (mdns_transport, bt_transport, cmd_tx) = {
                let net = network.lock().await;
                let cmd_tx = net.primary.cmd_tx.clone();

                (net.mdns_transport.clone(), net.bt_transport.clone(), cmd_tx)
            };

            let result = if method == "mdns" {
                nearby_connect(
                    &peer_id,
                    &session_name,
                    &*mdns_transport,
                    db,
                    &app_event_tx,
                    &cmd_tx,
                )
                .await
            } else {
                nearby_connect(
                    &peer_id,
                    &session_name,
                    &*bt_transport,
                    db,
                    &app_event_tx,
                    &cmd_tx,
                )
                .await
            };
            reply.send(result).ok();
        }

        CoreCommand::SendText {
            contact_id,
            text,
            reply_to,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let now = get_timestamp_secs()?;

                let msg = KursalMessage::Text(TextMessage {
                    id: MessageId::new(),
                    content: text,
                    timestamp: now,
                    reply_to,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let msg_id = send_message(msg, &contact, db, &cmd_tx, Some(&app_event_tx))
                    .await?
                    .ok_or_else(|| {
                        KursalError::Storage("text message was sent without an id".to_string())
                    })?;

                Ok(msg_id)
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::SendTypingIndicator { contact_id, reply } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let msg = KursalMessage::Typing;
                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                send_message(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::SendReadReceipts {
            contact_id,
            message_ids,
            reply,
        } => {
            let result = async {
                if !crate::storage::get_read_receipts_enabled(&db) {
                    return Ok(());
                }

                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let ids: Vec<MessageId> = message_ids
                    .iter()
                    .filter_map(|id| {
                        hex::decode(id)
                            .ok()
                            .and_then(|b| b.try_into().ok())
                            .map(MessageId)
                    })
                    .collect();
                if ids.is_empty() {
                    return Ok(());
                }

                let msg = KursalMessage::ReadReceipt(ReadReceipt { message_ids: ids });
                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                send_message(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::RotatePeerId { reply } => {
            let result = async {
                let new_identity = TransportIdentity::generate();

                let mut net = network.lock().await;
                net.start_rotation(new_identity, &db).await?;
                drop(net);

                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                let net = network.lock().await;
                net.announce_address_rotation(db.clone(), &net.primary.cmd_tx, &app_event_tx)
                    .await?;

                let next = net.secondary.clone();
                drop(net);

                if let Some(next) = next {
                    spawn_pointer_publish(db.clone(), next, app_event_tx.clone());
                }

                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                let mut net = network.lock().await;
                net.complete_rotation(&db).await?;
                drop(net);

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::ShareProfile {
            contact_id,
            display_name,
            avatar_bytes,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let user_id = UserId(user_id_bytes);
                let contact = crate::messaging::offline::update_contact(&db, &user_id, |c| {
                    c.profile_shared = true;
                    true
                })
                .await?
                .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                share_profile_with(
                    &contact,
                    display_name,
                    avatar_bytes,
                    db.clone(),
                    &cmd_tx,
                    Some(&app_event_tx),
                )
                .await;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::BroadcastProfile {
            display_name,
            avatar_bytes,
            reply,
        } => {
            let result = async {
                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                let contacts = Contact::load_all(&db)?;

                for contact in contacts {
                    if contact.profile_shared {
                        share_profile_with(
                            &contact,
                            display_name.clone(),
                            avatar_bytes.clone(),
                            db.clone(),
                            &cmd_tx,
                            Some(&app_event_tx),
                        )
                        .await;
                    }
                }

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::RemoveContact { contact_id, reply } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let user_id = UserId(user_id_bytes);

                let existing = Contact::load(&db, &user_id)?;
                if let Some(contact) = existing {
                    let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                    let _ = send_message(
                        KursalMessage::ContactTerminate,
                        &contact,
                        db.clone(),
                        &cmd_tx,
                        None,
                    )
                    .await;
                    let _ = cmd_tx
                        .send(SwarmCommand::ContactRemoved {
                            peer_id: contact.peer_id.clone(),
                        })
                        .await;
                }

                remove_contact_transfers(db.clone(), &user_id).await?;
                Contact::delete(&db, &user_id)?;

                app_event_tx
                    .send(AppEvent::ContactRemoved {
                        contact_id: user_id,
                    })
                    .await
                    .ok_kursal(KursalError::Network)?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::DeleteLocalMessage {
            contact_id,
            message_id,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let user_id = UserId(user_id_bytes);

                let id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".into()))?;
                let msg_id = MessageId(id_bytes);

                StoredMessage::delete(&db, &user_id, &msg_id)?;
                crate::messaging::offline::discard_queued(&db, &user_id, &msg_id).await?;
                crate::messaging::offline::clear_pending_ack(&db, &user_id, &msg_id).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::RetryMessage {
            contact_id,
            message_id,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let user_id = UserId(user_id_bytes);

                let id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".into()))?;
                let msg_id = MessageId(id_bytes);

                let contact = Contact::load(&db, &user_id)?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let Some(mut message) = StoredMessage::load(&db, &user_id, &msg_id)? else {
                    return Ok(());
                };
                let original_ts = message.timestamp;
                message.status = MessageStatus::Sending;
                message.save(&db)?;
                let payload = message.payload;

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                send_message(payload, &contact, db.clone(), &cmd_tx, Some(&app_event_tx)).await?;

                if let Some(mut restored) = StoredMessage::load(&db, &user_id, &msg_id)? {
                    restored.timestamp = original_ts;
                    restored.save(&db)?;
                }

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::DeleteMessage {
            contact_id,
            message_id,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let msg = KursalMessage::MessageDelete(MessageDelete {
                    target_id: MessageId(message_id_bytes),
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let (_msg_id, queued_offline) =
                    send_message_tracked(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(queued_offline)
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::EditMessage {
            contact_id,
            message_id,
            new_content,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let now = get_timestamp_secs()?;

                let msg = KursalMessage::MessageEdit(MessageEdit {
                    target_id: MessageId(message_id_bytes),
                    edited_at: now,
                    new_content,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let (_msg_id, queued_offline) =
                    send_message_tracked(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(queued_offline)
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::PinMessage {
            contact_id,
            message_id,
            pinned,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let msg = KursalMessage::MessagePin(MessagePin {
                    target_id: MessageId(message_id_bytes),
                    pinned,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let (_msg_id, queued_offline) =
                    send_message_tracked(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(queued_offline)
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::ReactionAdd {
            contact_id,
            message_id,
            emoji,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let now = get_timestamp_secs()?;

                let msg = KursalMessage::ReactionAdd(ReactionAdd {
                    target_id: MessageId(message_id_bytes),
                    timestamp: now,
                    emoji,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let (_msg_id, queued_offline) =
                    send_message_tracked(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(queued_offline)
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::ReactionRemove {
            contact_id,
            message_id,
            emoji,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let msg = KursalMessage::ReactionRemove(ReactionRemove {
                    target_id: MessageId(message_id_bytes),
                    emoji,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let (_msg_id, queued_offline) =
                    send_message_tracked(msg, &contact, db, &cmd_tx, Some(&app_event_tx)).await?;

                Ok(queued_offline)
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::SendFileOffer {
            contact_id,
            file_path,
            app_data_dir,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&db, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let metadata = std::fs::metadata(&file_path).map_err(KursalError::Io)?;

                if !metadata.is_file() {
                    return Err(KursalError::Storage(
                        "Cannot transfer folders or symlinks".to_string(),
                    ));
                }

                let filename = Path::new(&file_path)
                    .file_name()
                    .ok_or(KursalError::Storage("File name not found".to_string()))?
                    .to_string_lossy()
                    .to_string();

                let offer_id = MessageId::new();
                let offer_hex = hex::encode(offer_id.0);

                let staged = stage_outgoing(
                    app_data_dir,
                    contact_id.clone(),
                    offer_hex.clone(),
                    PathBuf::from(&file_path),
                    filename.clone(),
                )
                .await?;

                let send_path = match staged {
                    Some(path) => path.to_string_lossy().to_string(),
                    None => file_path,
                };

                let size_bytes = std::fs::metadata(&send_path)
                    .map_err(KursalError::Io)?
                    .len();

                let mut my_random = [0u8; 32];
                OsRng
                    .try_fill_bytes(&mut my_random)
                    .ok_kursal(KursalError::Crypto)?;

                let now = get_timestamp_secs()?;

                let hash_path = send_path.clone();
                let hash = tokio::task::spawn_blocking(move || hash_file(&hash_path))
                    .await
                    .ok_kursal(KursalError::Storage)??;

                let stored_path = send_path.clone();

                let entry = FileTransferEntry {
                    path: send_path,
                    my_random,
                    shared_at: now,
                    last_accessed_at: None,
                    size_bytes,
                };

                db.raw_write(
                    TABLE_FILE_TRANSFERS,
                    &format!("send:{contact_id}:{}", hex::encode(offer_id.0)),
                    &entry.serialize().ok_kursal(KursalError::Storage)?,
                )?;

                let msg = KursalMessage::FileOffer(FileOffer {
                    id: offer_id,
                    filename,
                    size_bytes,
                    random: my_random,
                    hash,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let msg_id = send_message(msg, &contact, db, &cmd_tx, Some(&app_event_tx))
                    .await?
                    .ok_or_else(|| {
                        KursalError::Storage("file offer was sent without an id".to_string())
                    })?;

                Ok((msg_id, size_bytes, stored_path))
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::AcceptFileOffer {
            contact_id,
            offer_id,
            save_path,
            reply,
        } => {
            let result = async {
                let recv_key = format!("recv:{contact_id}:{offer_id}");
                let prog_key = format!("recvprog:{contact_id}:{offer_id}");

                let entry_bytes =
                    db.raw_read(TABLE_FILE_TRANSFERS, &recv_key)?
                        .ok_or(KursalError::Storage(
                            "Could not find file offer".to_string(),
                        ))?;

                let entry = FileIncomingEntry::deserialize(&entry_bytes)?;

                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;
                let contact_uid = UserId(user_id_bytes);

                let offer_id_bytes: [u8; 16] = hex::decode(&offer_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid offer id length".into()))?;

                let existing_prog = db.raw_read(TABLE_FILE_TRANSFERS, &prog_key)?;

                let (my_random, received_chunks) = if let Some(bytes) = existing_prog {
                    let prog = FileReceiveEntry::deserialize(&bytes)?;
                    (prog.my_random, prog.received_chunks)
                } else {
                    let chunk_count =
                        usize::try_from(entry.file_size.div_ceil(FILE_CHUNK_SIZE as u64))
                            .ok_kursal(KursalError::Storage)?;
                    let bitset_len = chunk_count.div_ceil(8);

                    if let Some(available) = available_space(Path::new(&save_path))
                        && entry.file_size > available
                    {
                        return Err(KursalError::InsufficientSpace {
                            needed: entry.file_size,
                            available,
                        });
                    }

                    let file = std::fs::File::create(&save_path).map_err(KursalError::Io)?;
                    file.set_len(entry.file_size).map_err(KursalError::Io)?;

                    if entry.file_size == 0 {
                        let hash_path = save_path.clone();
                        let actual = tokio::task::spawn_blocking(move || hash_file(&hash_path))
                            .await
                            .ok_kursal(KursalError::Storage)??;
                        db.raw_delete(TABLE_FILE_TRANSFERS, &recv_key)?;
                        if actual == entry.hash {
                            app_event_tx
                                .send(AppEvent::FileReceived {
                                    contact_id: contact_uid.clone(),
                                    transfer_id: MessageId(offer_id_bytes),
                                    save_path: save_path.clone(),
                                })
                                .await
                                .ok();
                        } else {
                            app_event_tx
                                .send(AppEvent::FileTransferFailed {
                                    contact_id: contact_uid.clone(),
                                    transfer_id: MessageId(offer_id_bytes),
                                    reason: "hash_mismatch".to_string(),
                                })
                                .await
                                .ok();
                        }
                        return Ok(());
                    }

                    let mut my_random = [0u8; 32];
                    OsRng
                        .try_fill_bytes(&mut my_random)
                        .ok_kursal(KursalError::Crypto)?;

                    let key = derive_stream_key(entry.their_random, my_random)?;
                    let received_chunks = vec![0u8; bitset_len];

                    let prog = FileReceiveEntry {
                        key,
                        my_random,
                        file_size: entry.file_size,
                        save_path: save_path.clone(),
                        received_chunks: received_chunks.clone(),
                        expected_hash: entry.hash,
                        created_at: get_timestamp_secs()?,
                    };
                    db.raw_write(TABLE_FILE_TRANSFERS, &prog_key, &prog.serialize()?)?;

                    (my_random, received_chunks)
                };

                let contact = Contact::load(&db, &contact_uid)?
                    .ok_or_else(|| KursalError::Identity("Unknown contact".to_string()))?;

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                send_message(
                    KursalMessage::FileAccept(FileAccept {
                        offer_id: MessageId(offer_id_bytes),
                        random: my_random,
                        received_chunks,
                    }),
                    &contact,
                    db.clone(),
                    &cmd_tx,
                    Some(&app_event_tx),
                )
                .await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::CancelFileTransfer {
            contact_id,
            offer_id,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let contact_uid = UserId(user_id_bytes);

                let offer_id_bytes: [u8; 16] = hex::decode(&offer_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid offer id length".into()))?;

                let contact = Contact::load(&db, &contact_uid)?
                    .ok_or_else(|| KursalError::Identity("Unknown contact".to_string()))?;

                apply_cancel(db.clone(), &contact_uid, offer_id_bytes, &app_event_tx).await?;

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                send_message(
                    KursalMessage::FileCancel(FileCancel {
                        offer_id: MessageId(offer_id_bytes),
                    }),
                    &contact,
                    db.clone(),
                    &cmd_tx,
                    Some(&app_event_tx),
                )
                .await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::AnnounceAddresses => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            if let Err(err) =
                crate::network::rotation::announce_addresses_to_offline(db, &cmd_tx, &app_event_tx)
                    .await
            {
                log::warn!("[announce] periodic announce failed: {err}");
            }
        }

        CoreCommand::FlushOffline { contact_id, reply } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .ok_kursal(KursalError::Crypto)?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let user_id = UserId(user_id_bytes);

                if Contact::load(&db, &user_id)?.is_none() {
                    return Err(KursalError::Storage("Contact not found".into()));
                }

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                let db_for_flush = db.clone();
                let event_tx_for_flush = app_event_tx.clone();
                tokio::task::spawn_local(async move {
                    match crate::messaging::offline::deliver_queue_direct(
                        &user_id,
                        &cmd_tx,
                        db_for_flush.clone(),
                        Some(&event_tx_for_flush),
                    )
                    .await
                    {
                        Ok(0) => {}
                        Ok(n) => log::info!("[offline] manual drain delivered {n} directly"),
                        Err(err) => log::warn!("[offline] manual drain failed: {err}"),
                    }

                    if let Err(err) = crate::messaging::offline::flush_now(
                        &user_id,
                        &cmd_tx,
                        db_for_flush.clone(),
                        Some(&event_tx_for_flush),
                    )
                    .await
                    {
                        log::warn!("[offline] manual flush failed: {err}");
                    }
                });

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }
        CoreCommand::AddCustomNode { addr, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::api::nodes::add_custom_node(addr, db, cmd_tx).await;
            reply.send(result).ok();
        }
        CoreCommand::RemoveCustomNode { addr, reply } => {
            let result = crate::api::nodes::remove_custom_node(addr, db).await;
            reply.send(result).ok();
        }
        CoreCommand::DialAddress { addr, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::api::nodes::dial_address(addr, cmd_tx).await;
            reply.send(result).ok();
        }
        CoreCommand::NetworkStatus { reply } => {
            let (cmd_tx, port) = {
                let guard = network.lock().await;
                (guard.primary.cmd_tx.clone(), guard.primary.port)
            };
            let result = crate::api::nodes::network_status(cmd_tx, port).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::StartCall { contact_id, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::start(&contact_id, db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::AcceptCall { reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::accept(db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::DeclineCall { reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::decline(db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::HangupCall { reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::hangup(db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::StartVideo { reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::start_video(db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::ListCameras { reply } => {
            reply
                .send(Ok(crate::call::manager::list_cameras().await))
                .ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::RequestLocalKeyframe { reply } => {
            crate::call::capture::request_keyframe();
            reply.send(Ok(())).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::SetCamera { camera_id, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result =
                crate::call::manager::set_camera(camera_id, db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::RefreshCameraRotation { angle, reply } => {
            crate::call::manager::refresh_camera_rotation(angle).await;
            reply.send(Ok(())).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::StopVideo { reason, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::stop_video(reason, db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::RequestVideoKeyframe { reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result =
                crate::call::manager::request_video_keyframe(db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::SetDeafen { deafened, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result =
                crate::call::manager::set_deafen(deafened, db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::SetMute { muted, reply } => {
            let cmd_tx = network.lock().await.primary.cmd_tx.clone();
            let result = crate::call::manager::set_mute(muted, db, &cmd_tx, &app_event_tx).await;
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::SetAudioDevice { kind, name, reply } => {
            let result = crate::call::manager::set_audio_device(kind.clone(), name.clone()).await;
            if result.is_ok() {
                let guard = &*db;
                let _ = match kind.as_str() {
                    "input" => crate::storage::set_audio_input_device(guard, name),
                    "output" => crate::storage::set_audio_output_device(guard, name),
                    _ => Ok(()),
                };
            }
            reply.send(result).ok();
        }

        #[cfg(feature = "calls")]
        CoreCommand::ListAudioDevices { reply } => {
            reply
                .send(Ok(crate::call::manager::list_audio_devices()))
                .ok();
        }
    }
}
