use kursal_core::storage::SharedDatabase;
use kursal_core::storage::files_list_shared;
use kursal_core::storage::filetransfer::{OUTGOING_PENDING, outgoing_pending_dir, outgoing_root};
use std::collections::HashSet;
use std::path::Path;

pub async fn sweep(db: SharedDatabase, app_data_dir: &Path) {
    let shared = match files_list_shared(&*db.0.lock().await) {
        Ok(shared) => shared,
        Err(err) => {
            log::warn!("Outgoing sweep skipped, could not list shares: {err}");
            return;
        }
    };

    let live: HashSet<(String, String)> = shared
        .iter()
        .filter_map(|entry| {
            let mut parts = entry.id.split(':');
            match (parts.next(), parts.next(), parts.next()) {
                (Some("send"), Some(contact), Some(offer)) => {
                    Some((contact.to_string(), offer.to_string()))
                }
                _ => None,
            }
        })
        .collect();

    let _ = std::fs::remove_dir_all(outgoing_pending_dir(app_data_dir));

    prune_orphaned_offers(&outgoing_root(app_data_dir), &live);
}

fn prune_orphaned_offers(root: &Path, live: &HashSet<(String, String)>) {
    let Ok(contacts) = root.read_dir() else {
        return;
    };

    for contact in contacts.flatten() {
        let contact_name = contact.file_name().to_string_lossy().to_string();
        if contact_name == OUTGOING_PENDING || !contact.path().is_dir() {
            continue;
        }

        let Ok(offers) = contact.path().read_dir() else {
            continue;
        };

        let mut kept = 0usize;
        for offer in offers.flatten() {
            let offer_name = offer.file_name().to_string_lossy().to_string();
            if live.contains(&(contact_name.clone(), offer_name)) {
                kept += 1;
            } else {
                let _ = std::fs::remove_dir_all(offer.path());
            }
        }

        if kept == 0 {
            let _ = std::fs::remove_dir(contact.path());
        }
    }
}
