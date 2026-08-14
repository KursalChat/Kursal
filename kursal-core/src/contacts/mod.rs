use crate::{
    KursalError, Result,
    crypto::DEVICE_ID,
    identity::UserId,
    messaging::offline::OfflineState,
    storage::{Database, TABLE_CONTACTS, TABLE_MESSAGES, TABLE_SESSIONS},
    sync::RwLockExt,
};
use libsignal_protocol::ProtocolAddress;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

#[derive(Default)]
struct Roster {
    by_user: HashMap<UserId, Contact>,
    by_peer: HashMap<String, UserId>,
}

impl Roster {
    fn insert(&mut self, contact: Contact) {
        if let Some(previous) = self.by_user.get(&contact.user_id)
            && previous.peer_id != contact.peer_id
        {
            self.by_peer.remove(&previous.peer_id);
        }
        self.by_peer
            .insert(contact.peer_id.clone(), contact.user_id.clone());
        self.by_user.insert(contact.user_id.clone(), contact);
    }

    fn remove(&mut self, user_id: &UserId) {
        if let Some(contact) = self.by_user.remove(user_id) {
            self.by_peer.remove(&contact.peer_id);
        }
    }
}

// contact mirrored in memory
static ROSTERS: LazyLock<RwLock<HashMap<u64, Arc<RwLock<Roster>>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn roster(db: &Database) -> Result<Arc<RwLock<Roster>>> {
    if let Some(existing) = ROSTERS.read_recover().get(&db.id()) {
        return Ok(existing.clone());
    }

    let mut loaded = Roster::default();
    for (_, bytes) in db.raw_readall(TABLE_CONTACTS)? {
        match Contact::deserialize(&bytes) {
            Ok(contact) => loaded.insert(contact),
            Err(err) => log::warn!("[contacts] skipping undeserializable contact: {err}"),
        }
    }

    let mut rosters = ROSTERS.write_recover();
    Ok(rosters
        .entry(db.id())
        .or_insert_with(|| Arc::new(RwLock::new(loaded)))
        .clone())
}

pub struct ContactRoute {
    pub user_id: UserId,
    pub peer_id: String,
    pub known_addresses: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Contact {
    pub user_id: UserId,
    pub peer_id: String,
    pub display_name: String,
    pub avatar: Option<String>,
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
        roster(db)?.write_recover().insert(self.clone());

        Ok(())
    }

    pub fn save_if_exists(&self, db: &Database) -> Result<()> {
        let roster = roster(db)?;
        if !roster.read_recover().by_user.contains_key(&self.user_id) {
            return Ok(());
        }

        let user_id = hex::encode(self.user_id.0);
        let serialized = self.serialize()?;
        db.raw_write(TABLE_CONTACTS, &user_id, &serialized)?;
        roster.write_recover().insert(self.clone());

        Ok(())
    }

    pub fn load(db: &Database, user_id: &UserId) -> Result<Option<Self>> {
        Ok(roster(db)?.read_recover().by_user.get(user_id).cloned())
    }

    pub fn load_all(db: &Database) -> Result<Vec<Self>> {
        Ok(roster(db)?
            .read_recover()
            .by_user
            .values()
            .cloned()
            .collect())
    }

    pub fn routes(db: &Database) -> Result<Vec<ContactRoute>> {
        Ok(roster(db)?
            .read_recover()
            .by_user
            .values()
            .map(|c| ContactRoute {
                user_id: c.user_id.clone(),
                peer_id: c.peer_id.clone(),
                known_addresses: c.known_addresses.clone(),
            })
            .collect())
    }

    pub fn find_by_peer_id(db: &Database, peer_id: &str) -> Result<Option<Self>> {
        let roster = roster(db)?;
        let roster = roster.read_recover();

        Ok(roster
            .by_peer
            .get(peer_id)
            .and_then(|user_id| roster.by_user.get(user_id))
            .cloned())
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
        let address = ProtocolAddress::new(contact_id.clone(), DEVICE_ID);

        db.raw_delete(TABLE_CONTACTS, &contact_id)?;
        roster(db)?.write_recover().remove(user_id);
        db.raw_delete_prefix(TABLE_MESSAGES, &format!("{contact_id}:"))?;
        db.raw_delete(TABLE_SESSIONS, &address.to_string())?;
        crate::storage::delete_contact_meta(db, &contact_id)?;
        crate::storage::conversation::delete_for_contact(db, &contact_id)?;

        Ok(())
    }
}
