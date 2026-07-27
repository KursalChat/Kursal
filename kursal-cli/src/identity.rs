use libp2p::identity::Keypair;
use std::path::Path;

pub fn save_keypair(keypair: &Keypair, path: &Path) -> std::io::Result<()> {
    let bytes = keypair
        .to_protobuf_encoding()
        .expect("keypair serialization failed");
    std::fs::write(path, bytes)
}

pub fn load_keypair(path: &Path) -> std::io::Result<Option<Keypair>> {
    if !path.exists() {
        return Ok(None);
    }

    let bytes = std::fs::read(path)?;

    let keypair = Keypair::from_protobuf_encoding(&bytes).expect("keypair deserialization failed");

    Ok(Some(keypair))
}

pub fn load_or_generate(path: &Path) -> std::io::Result<Keypair> {
    if let Some(keypair) = load_keypair(path)? {
        log::info!("[identity] loaded existing keypair");
        return Ok(keypair);
    }

    log::info!("[identity] generating new keypair");
    let keypair = Keypair::generate_ed25519();
    save_keypair(&keypair, path)?;

    Ok(keypair)
}
