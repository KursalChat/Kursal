use crate::{
    KursalError, Result,
    identity::UserId,
    messaging::offline::OfflineState,
    storage::{Database, TABLE_CONTACTS, TABLE_MESSAGES, TABLE_SESSIONS},
};
use libsignal_protocol::{DeviceId, ProtocolAddress};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

static PEER_ID_CACHE: LazyLock<RwLock<HashMap<String, UserId>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Serialize, Deserialize, Clone)]
pub struct Contact {
    pub user_id: UserId,
    pub peer_id: String,
    pub display_name: String,
    pub avatar_bytes: Option<Vec<u8>>,
    pub identity_pub_key: Vec<u8>,
    pub dilithium_pub_key: Vec<u8>,
    pub known_addresses: Vec<String>,
    pub verified: bool,
    pub profile_shared: bool,
    pub blocked: bool,
    pub created_at: u64,
    pub offline: OfflineState,
}

impl Contact {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    pub fn save(&self, db: &Database) -> Result<()> {
        let user_id = hex::encode(self.user_id.0);
        let serialized = self.serialize()?;

        db.raw_write(TABLE_CONTACTS, &user_id, &serialized)?;

        Ok(())
    }

    pub fn save_if_exists(&self, db: &Database) -> Result<()> {
        let user_id = hex::encode(self.user_id.0);
        if db.raw_read(TABLE_CONTACTS, &user_id)?.is_none() {
            return Ok(());
        }
        let serialized = self.serialize()?;
        db.raw_write(TABLE_CONTACTS, &user_id, &serialized)?;

        Ok(())
    }

    pub fn load(db: &Database, user_id: &UserId) -> Result<Option<Self>> {
        let user_id = hex::encode(user_id.0);
        let raw = db.raw_read(TABLE_CONTACTS, &user_id)?;

        match raw {
            Some(v) => match Contact::deserialize(&v) {
                Ok(contact) => Ok(Some(contact)),
                Err(e) => {
                    log::warn!("Failed to deserialize contact {user_id}, treating as absent: {e}");
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    pub fn load_all(db: &Database) -> Result<Vec<Self>> {
        let all = db.raw_readall(TABLE_CONTACTS)?;

        let contacts = all
            .iter()
            .filter_map(|(_, el)| match Contact::deserialize(el) {
                Ok(contact) => Some(contact),
                Err(e) => {
                    log::warn!("Failed to deserialize contact: {e}");
                    None
                }
            })
            .collect();

        Ok(contacts)
    }

    pub fn find_by_peer_id(db: &Database, peer_id: &str) -> Result<Option<Self>> {
        let cached = PEER_ID_CACHE.read().unwrap().get(peer_id).cloned();
        if let Some(user_id) = cached
            && let Some(contact) = Contact::load(db, &user_id)?
            && contact.peer_id == peer_id
        {
            return Ok(Some(contact));
        }

        let all = Contact::load_all(db)?;
        {
            let mut cache = PEER_ID_CACHE.write().unwrap();
            cache.clear();
            for contact in &all {
                cache.insert(contact.peer_id.clone(), contact.user_id.clone());
            }
        }

        Ok(all.into_iter().find(|c| c.peer_id == peer_id))
    }

    pub fn set_verified(db: &Database, user_id: &UserId) -> Result<()> {
        let mut contact = Contact::load(db, user_id)?
            .ok_or(KursalError::Storage("Contact not found".to_string()))?;

        contact.verified = true;
        contact.save(db)?;

        Ok(())
    }

    pub fn set_blocked(db: &Database, user_id: &UserId, value: bool) -> Result<()> {
        let mut contact = Contact::load(db, user_id)?
            .ok_or(KursalError::Storage("Contact not found".to_string()))?;

        contact.blocked = value;
        contact.save(db)?;

        Ok(())
    }

    pub fn set_addresses(db: &Database, user_id: &UserId, addresses: Vec<String>) -> Result<()> {
        let mut contact = Contact::load(db, user_id)?
            .ok_or(KursalError::Storage("Contact not found".to_string()))?;

        contact.known_addresses = addresses;
        contact.save(db)?;

        Ok(())
    }

    pub fn delete(db: &Database, user_id: &UserId) -> Result<()> {
        let contact_id = hex::encode(user_id.0);
        let address = ProtocolAddress::new(contact_id.clone(), DeviceId::new(1u8).unwrap());

        db.raw_delete(TABLE_CONTACTS, &contact_id)?;
        db.raw_delete_prefix(TABLE_MESSAGES, &format!("{contact_id}:"))?;
        db.raw_delete(TABLE_SESSIONS, &address.to_string())?;
        crate::storage::delete_contact_meta(db, &contact_id)?;
        crate::storage::conversation::delete_for_contact(db, &contact_id)?;

        Ok(())
    }
}
