use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::{
        AppEvent, apply_address_announce,
        file_transfers::{
            FileIncomingEntry, FileTransferEntry, apply_cancel, spawn_send_file_chunks,
        },
        message_apply::{
            apply_delete, apply_edit, apply_pin, apply_reaction_add, apply_reaction_remove,
        },
        send_message,
    },
    contacts::Contact,
    crypto::{DEVICE_ID, messages::message_receive},
    first_contact::{FileTransferMessage, WireMessage, handle_fc_response, resolve_ack_waiter},
    identity::UserId,
    messaging::{
        StoredMessage,
        enums::{DeliveryReceipt, Direction, FileCancel, KursalMessage, MessageId, MessageStatus},
    },
    network::swarm::{MAX_MESSAGE_SIZE, NetworkEvent, SwarmCommand},
    storage::{
        SharedDatabase, TABLE_FILE_TRANSFERS,
        filetransfer::{
            download_path, get_auto_download_storage, get_auto_download_storage_for,
            sanitize_filename,
        },
        get_auto_accept_config, get_auto_download_config, get_contact_terminated,
        get_timestamp_secs, set_contact_terminated,
    },
    sync::LockExt,
};
use futures::AsyncReadExt;
use libp2p::PeerId;
use libsignal_protocol::ProtocolAddress;
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use tokio::{fs::create_dir_all, sync::mpsc};

pub async fn handle_incoming(
    from: PeerId,
    ciphertext: Vec<u8>,
    db: SharedDatabase,
    app_data_dir: &std::path::Path,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
    chunk_tx: &mpsc::Sender<(PeerId, FileTransferMessage)>,
) -> Result<()> {
    let peer_id_str = from.to_base58();

    let encrypted_payload = match bincode::deserialize::<WireMessage>(&ciphertext) {
        Ok(WireMessage::ContactResponse(response)) => {
            let _ = handle_fc_response(response, db.clone(), cmd_tx, event_tx).await;
            return Ok(());
        }
        Ok(WireMessage::FileTransfer(chunk)) => {
            let _ = chunk_tx.send((from, chunk)).await;
            return Ok(());
        }
        Ok(WireMessage::ContactAccepted(payload_id)) => {
            resolve_ack_waiter(payload_id, Ok(()));
            return Ok(());
        }
        Ok(WireMessage::ContactRejected { payload_id, reason }) => {
            resolve_ack_waiter(payload_id, Err(reason));
            return Ok(());
        }
        Ok(WireMessage::Terminate) => {
            let known = Contact::find_by_peer_id(&*db.clone().0.lock().await, &peer_id_str)?;
            if let Some(contact) = known {
                mark_terminated(&db, &contact.user_id, true, event_tx).await?;
            }
            return Ok(());
        }
        Ok(WireMessage::Encrypted(data)) => data,
        Err(_e) => {
            // log::debug!("Rejected invalid WireMessage from {}: {}", peer_id_str, e);
            return Ok(());
        }
    };

    let known = Contact::find_by_peer_id(&*db.clone().0.lock().await, &peer_id_str)?;
    let Some(contact) = known else {
        reply_terminate_once(from, cmd_tx).await;
        return Ok(());
    };

    if contact.blocked {
        return Ok(()); // ignore
    }

    let remote_address = ProtocolAddress::new(hex::encode(contact.user_id.0), DEVICE_ID);
    let now = get_timestamp_secs()?;

    let received = message_receive(db.clone(), &remote_address, &encrypted_payload).await?;
    let kmessage = KursalMessage::deserialize(&received)?;

    if matches!(kmessage, KursalMessage::ContactTerminate) {
        mark_terminated(&db, &contact.user_id, true, event_tx).await?;
        return Ok(());
    }
    mark_terminated(&db, &contact.user_id, false, event_tx).await?;

    match kmessage {
        KursalMessage::Typing => {
            event_tx
                .send(AppEvent::TypingIndicator {
                    contact_id: contact.user_id,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            return Ok(());
        }

        KursalMessage::MessagePin(ref pin) => {
            apply_pin(&contact, pin, &db, event_tx).await?;
        }

        KursalMessage::MessageEdit(ref edit) => {
            apply_edit(&contact, edit, &db, event_tx).await?;
        }

        KursalMessage::MessageDelete(ref del) => {
            apply_delete(&contact, del, &db, event_tx).await?;
        }

        KursalMessage::ReactionRemove(ref r) => {
            apply_reaction_remove(&contact, r, &db, event_tx).await?;
        }

        KursalMessage::ReactionAdd(ref r) => {
            apply_reaction_add(&contact, r, &db, event_tx, now).await?;
        }

        KursalMessage::Text(ref text) => {
            let msg_id = text.id;

            let stored = StoredMessage {
                id: msg_id,
                contact_id: contact.user_id.clone(),
                payload: kmessage,
                timestamp: now,
                direction: Direction::Received,
                status: MessageStatus::Delivered,
                raw_ciphertext: None,
                edited: false,
                pinned: false,
                reactions: Vec::with_capacity(0),
            };

            stored.save(&*db.clone().0.lock().await)?;

            event_tx
                .send(AppEvent::MessageReceived {
                    contact_id: contact.user_id.clone(),
                    message: stored,
                    via_offline: false,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            send_delivery_receipt(db.clone(), msg_id, &contact, cmd_tx).await?;
        }

        KursalMessage::DeliveryReceipt(receipt) => {
            let via_mailbox = crate::messaging::offline::ack_delivered(
                &contact.user_id,
                &receipt.message_id,
                &db,
            )
            .await?;
            crate::messaging::offline::clear_pending_ack(
                &db,
                &contact.user_id,
                &receipt.message_id,
            )
            .await?;

            let loaded = StoredMessage::load(
                &*db.clone().0.lock().await,
                &contact.user_id,
                &receipt.message_id,
            )?;
            if let Some(mut message) = loaded {
                message.status = if via_mailbox {
                    MessageStatus::OfflineDelivered
                } else {
                    MessageStatus::Delivered
                };
                message.save(&*db.clone().0.lock().await)?;

                event_tx
                    .send(AppEvent::DeliveryConfirmed {
                        contact_id: contact.user_id,
                        message_id: message.id,
                    })
                    .await
                    .ok_kursal(KursalError::Network)?;
            }
        }

        KursalMessage::ReadReceipt(receipt) => {
            let mut confirmed = Vec::with_capacity(receipt.message_ids.len());
            for message_id in receipt.message_ids {
                let loaded = StoredMessage::load(
                    &*db.clone().0.lock().await,
                    &contact.user_id,
                    &message_id,
                )?;
                if let Some(mut message) = loaded
                    && matches!(message.direction, Direction::Sent)
                    && !matches!(message.status, MessageStatus::Read)
                {
                    message.status = MessageStatus::Read;
                    message.save(&*db.clone().0.lock().await)?;
                    confirmed.push(message_id);
                }
            }
            if !confirmed.is_empty() {
                event_tx
                    .send(AppEvent::MessagesRead {
                        contact_id: contact.user_id,
                        message_ids: confirmed,
                    })
                    .await
                    .ok_kursal(KursalError::Network)?;
            }
        }

        KursalMessage::FileOffer(ref file) => {
            let filename = sanitize_filename(&file.filename);
            let offer_id = file.id;
            let size_bytes = file.size_bytes;
            let file_random = file.random;
            let file_hash = file.hash;

            let stored = StoredMessage {
                id: file.id,
                contact_id: contact.user_id.clone(),
                payload: kmessage,
                timestamp: now,
                direction: Direction::Received,
                status: MessageStatus::Delivered,
                raw_ciphertext: None,
                edited: false,
                pinned: false,
                reactions: Vec::with_capacity(0),
            };

            stored.save(&*db.0.lock().await)?;

            let mut autodownload = None;
            let auto_accept = get_auto_accept_config(&*db.0.lock().await);

            if auto_accept.size_cap_bytes >= size_bytes
                && ((auto_accept.mode == "verified" && contact.verified)
                    || auto_accept.mode == "all")
            {
                let auto_config = get_auto_download_config(&*db.0.lock().await);

                let contact_hex = hex::encode(contact.user_id.0);

                let size = if auto_config.scope == "all_contacts" {
                    get_auto_download_storage(app_data_dir)
                } else {
                    get_auto_download_storage_for(app_data_dir, &contact_hex)
                };

                match size {
                    Ok(size) => {
                        if size.saturating_add(size_bytes) <= auto_config.limit_bytes {
                            let path = download_path(
                                app_data_dir,
                                &contact_hex,
                                &hex::encode(offer_id.0),
                                &filename,
                            );

                            if let Some(parent) = path.parent() {
                                create_dir_all(parent).await.map_err(KursalError::Io)?;
                            }

                            autodownload = Some(path.to_string_lossy().into_owned());
                        }
                    }
                    Err(err) => {
                        log::error!("Could not get auto download folder size: {err}");
                    }
                }
            }

            let entry = FileIncomingEntry {
                their_random: file_random,
                file_size: size_bytes,
                hash: file_hash,
                created_at: now,
            };

            db.0.lock().await.raw_write(
                TABLE_FILE_TRANSFERS,
                &format!(
                    "recv:{}:{}",
                    hex::encode(contact.user_id.0),
                    hex::encode(offer_id.0)
                ),
                &entry.serialize()?,
            )?;

            event_tx
                .send(AppEvent::FileOffered {
                    contact_id: contact.user_id.clone(),
                    filename,
                    offer_id,
                    size_bytes,
                    autodownload,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            send_delivery_receipt(db.clone(), offer_id, &contact, cmd_tx).await?;
        }

        KursalMessage::FileAccept(file) => {
            let send_key = format!(
                "send:{}:{}",
                hex::encode(contact.user_id.0),
                hex::encode(file.offer_id.0)
            );

            let stored =
                db.0.lock()
                    .await
                    .raw_read(TABLE_FILE_TRANSFERS, &send_key)?;

            let Some(file_entry_bytes) = stored else {
                log::info!(
                    "[file] accept for unknown offer {}, telling peer to drop it",
                    hex::encode(file.offer_id.0)
                );
                send_message(
                    KursalMessage::FileCancel(FileCancel {
                        offer_id: file.offer_id,
                    }),
                    &contact,
                    db.clone(),
                    cmd_tx,
                    Some(event_tx),
                )
                .await?;
                return Ok(());
            };

            let mut file_entry = FileTransferEntry::deserialize(&file_entry_bytes)?;

            let now = get_timestamp_secs()?;
            file_entry.last_accessed_at = Some(now);
            db.0.lock().await.raw_write(
                TABLE_FILE_TRANSFERS,
                &send_key,
                &file_entry.serialize()?,
            )?;

            spawn_send_file_chunks(
                contact,
                file.offer_id,
                file_entry.path,
                file_entry.my_random,
                file.random,
                file.received_chunks,
                cmd_tx.clone(),
                event_tx.clone(),
            );
        }

        KursalMessage::FileCancel(cancel) => {
            apply_cancel(db.clone(), &contact.user_id, cancel.offer_id.0, event_tx).await?;
        }

        #[cfg(feature = "calls")]
        KursalMessage::CallSignal(signal) => {
            crate::call::manager::on_signal(&contact, signal, db.clone(), cmd_tx, event_tx).await?;
        }
        #[cfg(not(feature = "calls"))]
        KursalMessage::CallSignal(_) => {
            log::debug!("call signal ignored: built without calls feature");
        }

        KursalMessage::ProfileUpdate(profile) => {
            log::info!(
                "Received profile update: USERNAME: {} - AVATAR LEN: {}",
                profile.display_name,
                profile.avatar_bytes.as_ref().map(|e| e.len()).unwrap_or(0)
            );
            if profile.validate().is_err() {
                return Ok(());
            }

            let name = profile.display_name;
            let avatar = profile.avatar_bytes;
            if let Some(updated) =
                crate::messaging::offline::update_contact(&db, &contact.user_id, move |c| {
                    c.display_name = name;
                    c.avatar_bytes = avatar;
                    true
                })
                .await?
            {
                event_tx
                    .send(AppEvent::ContactUpdated { contact: updated })
                    .await
                    .ok_kursal(KursalError::Network)?;
            }
        }

        KursalMessage::AddressAnnounce(ref announce) => {
            apply_address_announce(
                &contact.user_id,
                announce.peer_id.clone(),
                announce.addresses.clone(),
                &db,
                cmd_tx,
                Some(event_tx),
            )
            .await?;
        }
        KursalMessage::CallRecord(_) | KursalMessage::ContactTerminate => {}
    }

    Ok(())
}

static TERMINATE_REPLIED: LazyLock<Mutex<HashSet<PeerId>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

const TERMINATE_REPLY_CAP: usize = 512;

async fn reply_terminate_once(peer: PeerId, cmd_tx: &mpsc::Sender<SwarmCommand>) {
    {
        let mut seen = TERMINATE_REPLIED.lock_recover();
        if seen.len() >= TERMINATE_REPLY_CAP || !seen.insert(peer) {
            return;
        }
    }

    let Ok(data) = bincode::serialize(&WireMessage::Terminate) else {
        return;
    };
    let _ = cmd_tx
        .send(SwarmCommand::SendMessage {
            peer_id: peer,
            data,
            addresses: Vec::with_capacity(0),
        })
        .await;
}

pub(crate) async fn mark_terminated(
    db: &SharedDatabase,
    contact_id: &UserId,
    terminated: bool,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let contact_hex = hex::encode(contact_id.0);
    {
        let guard = db.0.lock().await;
        if get_contact_terminated(&guard, &contact_hex) == terminated {
            return Ok(());
        }
        set_contact_terminated(&guard, &contact_hex, terminated)?;
    }

    event_tx
        .send(AppEvent::ContactTerminated {
            contact_id: contact_id.clone(),
            terminated,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    Ok(())
}

pub async fn handle_incoming_stream(
    peer_id: PeerId,
    mut stream: libp2p::Stream,
    event_tx: mpsc::Sender<NetworkEvent>,
    chunk_tx: mpsc::Sender<(PeerId, FileTransferMessage)>,
) {
    loop {
        let mut len_bytes = [0u8; 4];
        if stream.read_exact(&mut len_bytes).await.is_err() {
            break;
        }
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > MAX_MESSAGE_SIZE {
            log::warn!("oversized stream message from {peer_id}, closing");
            break;
        }

        let mut data = vec![0u8; len];
        if stream.read_exact(&mut data).await.is_err() {
            break;
        }

        match bincode::deserialize::<WireMessage>(&data) {
            Ok(WireMessage::FileTransfer(chunk)) => {
                if chunk_tx.send((peer_id, chunk)).await.is_err() {
                    break;
                }
            }
            _ => {
                let _ = event_tx
                    .send(NetworkEvent::MessageReceived {
                        from: peer_id,
                        data,
                    })
                    .await;
            }
        }
    }
    log::info!("[stream] incoming stream from {peer_id} closed");
}

pub async fn send_delivery_receipt(
    db: SharedDatabase,
    message_id: MessageId,
    contact: &Contact,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let receipt = KursalMessage::DeliveryReceipt(DeliveryReceipt { message_id });

    send_message(receipt, contact, db, cmd_tx, None).await?;

    Ok(())
}
