use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    identity::{
        generators::{generate_dilithium_keypair, generate_identity_keypair},
        keychain::{
            KeychainConfig, generate_master_secret, get_entry, load_master_secret,
            store_master_secret,
        },
    },
    storage::{Database, SharedDatabase, TABLE_IDENTITY_KEYS, TABLE_SETTINGS, derive_db_key},
};
use libp2p::PeerId;
use libsignal_protocol::IdentityKeyStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

pub mod generators;
pub mod keychain;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(pub [u8; 32]);
impl UserId {
    pub async fn local_user_id(db: SharedDatabase) -> Result<UserId> {
        let keypair = db
            .get_identity_key_pair()
            .await
            .ok_kursal(KursalError::Crypto)?;

        let serialized = keypair.public_key().serialize();
        let hash: [u8; 32] = Sha256::digest(&serialized).into();

        Ok(UserId(hash))
    }
}

pub async fn init(
    db_path: &Path,
    keychain_config: &KeychainConfig,
    app_data_dir: &Path,
) -> Result<SharedDatabase> {
    let master_key = load_or_create_master_key(db_path, keychain_config, app_data_dir)?;

    let mut db_enc = derive_db_key(&master_key)?;
    let mut db = Database::open(db_path, db_enc)?;
    db_enc.zeroize();

    verify_db_key(&db)?;
    ensure_identity(&mut db)?;

    crate::storage::avatars::init(app_data_dir)?;
    crate::storage::avatars::migrate(&db)?;
    crate::storage::avatars::prune_unreferenced(&db)?;

    Ok(SharedDatabase::from_db(db))
}

fn verify_db_key(db: &Database) -> Result<()> {
    const DB_CHECK_KEY: &str = "db_check";
    const DB_CHECK_VALUE: &[u8] = b"kursal-db-v1";

    match db.raw_read(TABLE_SETTINGS, DB_CHECK_KEY) {
        Ok(Some(value)) if value == DB_CHECK_VALUE => Ok(()),
        Ok(Some(_)) | Err(KursalError::Crypto(_)) => Err(KursalError::KeyMismatch),
        Ok(None) => {
            db.raw_write(TABLE_SETTINGS, DB_CHECK_KEY, DB_CHECK_VALUE)?;
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn load_or_create_master_key(
    db_path: &Path,
    keychain_config: &KeychainConfig,
    app_data_dir: &Path,
) -> Result<Zeroizing<Vec<u8>>> {
    let entry = get_entry(keychain_config)?;

    if let Some(master_key) = load_master_secret(keychain_config, &entry, app_data_dir)? {
        return Ok(Zeroizing::new(master_key));
    }

    if db_path.exists() {
        return Err(KursalError::Storage(
            "Database exists but its master key is missing from the keychain.".to_string(),
        ));
    }

    let mut generated = generate_master_secret()?;
    store_master_secret(&generated, keychain_config, &entry, app_data_dir)?;

    let key = Zeroizing::new(generated.to_vec());
    generated.zeroize();

    Ok(key)
}

fn ensure_identity(db: &mut Database) -> Result<()> {
    let has_identity = db
        .raw_read(TABLE_IDENTITY_KEYS, "local_identity")?
        .is_some()
        && db.raw_read(TABLE_SETTINGS, "registration_id")?.is_some();
    if !has_identity {
        generate_identity_keypair(db)?;
    }

    let has_dilithium = db.raw_read(TABLE_SETTINGS, "dilithium_public")?.is_some()
        && db.raw_read(TABLE_SETTINGS, "dilithium_secret")?.is_some();
    if !has_dilithium {
        generate_dilithium_keypair(db)?;
    }

    Ok(())
}

pub fn init_transport(db: &Database) -> Result<TransportIdentity> {
    let _ = db.raw_delete(TABLE_SETTINGS, "transport_identity_next");
    match TransportIdentity::load(db)? {
        Some(id) => Ok(id),
        None => {
            let id = TransportIdentity::generate();
            id.save(db)?;
            Ok(id)
        }
    }
}

pub struct TransportIdentity {
    pub keypair: libp2p::identity::Keypair,
    pub peer_id: PeerId,
}

impl TransportIdentity {
    pub fn generate() -> Self {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from_public_key(&keypair.public());

        Self { keypair, peer_id }
    }

    fn save_under(&self, db: &Database, key: &str) -> Result<()> {
        let serialized = self
            .keypair
            .to_protobuf_encoding()
            .ok_kursal(KursalError::Crypto)?;

        db.raw_write(TABLE_SETTINGS, key, &serialized)?;

        Ok(())
    }

    pub fn save(&self, db: &Database) -> Result<()> {
        self.save_under(db, "transport_identity")
    }

    pub fn save_next(&self, db: &Database) -> Result<()> {
        self.save_under(db, "transport_identity_next")
    }

    fn load_under(db: &Database, key: &str) -> Result<Option<Self>> {
        let fetched = db.raw_read(TABLE_SETTINGS, key)?;

        match fetched {
            Some(b) => {
                let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&b)
                    .ok_kursal(KursalError::Crypto)?;
                let peer_id = PeerId::from_public_key(&keypair.public());

                Ok(Some(Self { keypair, peer_id }))
            }
            None => Ok(None),
        }
    }

    pub fn load(db: &Database) -> Result<Option<Self>> {
        TransportIdentity::load_under(db, "transport_identity")
    }

    pub fn load_next(db: &Database) -> Result<Option<Self>> {
        TransportIdentity::load_under(db, "transport_identity_next")
    }

    pub fn promote_next(db: &Database) -> Result<Option<Self>> {
        let Some(next) = TransportIdentity::load_next(db)? else {
            return Ok(None);
        };
        next.save(db)?;
        db.raw_delete(TABLE_SETTINGS, "transport_identity_next")?;
        Ok(Some(next))
    }
}

pub fn security_code(
    local_identity_pub: &[u8],
    local_dilithium_pub: &[u8],
    remote_identity_pub: &[u8],
    remote_dilithium_pub: &[u8],
) -> String {
    let mut id = [local_identity_pub, remote_identity_pub];
    id.sort();
    let id = id.concat();

    let mut dili = [local_dilithium_pub, remote_dilithium_pub];
    dili.sort();
    let dili = dili.concat();

    let both = [id, dili].concat();
    let hash: [u8; 32] = Sha256::digest(both).into();

    hash.chunks_exact(4)
        .take(8)
        .map(|c| {
            let chunk = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
            format!("{:04}", chunk % 10000)
        })
        .collect::<Vec<_>>()
        .join(" ")
}
