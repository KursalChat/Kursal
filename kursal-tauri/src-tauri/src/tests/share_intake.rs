use crate::share_intake::{ShareFile, SharePayload, discard_in, take_pending_in};
use kursal_core::storage::filetransfer::outgoing_pending_dir;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct Dirs {
    root: PathBuf,
    app_data: PathBuf,
}

fn temp_dirs() -> Dirs {
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("kursal-share-test-{}-{unique}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);

    let root = base.join("drop-box");
    let app_data = base.join("app-data");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&app_data).unwrap();

    Dirs { root, app_data }
}

fn stage(root: &Path, id: &str, filenames: &[&str], text: Option<&str>) -> PathBuf {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).unwrap();

    let files: Vec<ShareFile> = filenames
        .iter()
        .map(|name| {
            let path = dir.join(name);
            std::fs::write(&path, b"payload").unwrap();
            ShareFile {
                path: path.to_string_lossy().into_owned(),
                filename: (*name).to_string(),
                size_bytes: 7,
            }
        })
        .collect();

    let manifest = SharePayload {
        id: String::new(),
        files,
        text: text.map(str::to_string),
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    dir
}

#[test]
fn drained_files_are_moved_into_the_outgoing_pending_dir() {
    let d = temp_dirs();
    let dir = stage(&d.root, "aaa", &["photo.jpg"], None);

    let payloads = take_pending_in(&d.root, &d.app_data).unwrap();

    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].id, "aaa");
    assert_eq!(payloads[0].files[0].filename, "photo.jpg");

    // send_file_offer only takes ownership of files living under the pending
    // dir; anywhere else it serves the offer from the original path.
    let staged = Path::new(&payloads[0].files[0].path);
    assert!(staged.is_file());
    assert!(staged.starts_with(outgoing_pending_dir(&d.app_data)));
    assert_eq!(std::fs::read(staged).unwrap(), b"payload");

    // The drop-box copy is gone, so nothing is left to leak.
    assert!(!dir.exists());
}

#[test]
fn each_file_gets_its_own_pending_dir() {
    let d = temp_dirs();
    stage(&d.root, "aaa", &["one.jpg", "two.jpg"], None);

    let payloads = take_pending_in(&d.root, &d.app_data).unwrap();
    let files = &payloads[0].files;

    assert_eq!(files.len(), 2);
    let first = Path::new(&files[0].path).parent().unwrap();
    let second = Path::new(&files[1].path).parent().unwrap();

    // Sending one file deletes its parent dir, so a shared parent would take
    // the sibling files down with it.
    assert_ne!(first, second);
}

#[test]
fn drain_is_idempotent() {
    let d = temp_dirs();
    stage(&d.root, "aaa", &["photo.jpg"], None);

    assert_eq!(take_pending_in(&d.root, &d.app_data).unwrap().len(), 1);
    assert!(take_pending_in(&d.root, &d.app_data).unwrap().is_empty());
}

#[test]
fn ignores_directory_without_manifest() {
    let d = temp_dirs();
    let dir = d.root.join("incomplete");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("half-copied.bin"), b"partial").unwrap();

    assert!(take_pending_in(&d.root, &d.app_data).unwrap().is_empty());
    assert!(dir.join("half-copied.bin").is_file());
}

#[test]
fn text_only_payload_survives() {
    let d = temp_dirs();
    stage(&d.root, "aaa", &[], Some("https://kursal.chat"));

    let payloads = take_pending_in(&d.root, &d.app_data).unwrap();

    assert_eq!(payloads.len(), 1);
    assert!(payloads[0].files.is_empty());
    assert_eq!(payloads[0].text.as_deref(), Some("https://kursal.chat"));
}

#[test]
fn drops_payload_whose_files_vanished() {
    let d = temp_dirs();
    let dir = stage(&d.root, "aaa", &["photo.jpg"], None);
    std::fs::remove_file(dir.join("photo.jpg")).unwrap();

    assert!(take_pending_in(&d.root, &d.app_data).unwrap().is_empty());
    assert!(!dir.exists());
}

#[test]
fn rejects_files_outside_the_root() {
    let d = temp_dirs();
    let outside = d.app_data.join("secret.txt");
    std::fs::write(&outside, b"nope").unwrap();

    let dir = d.root.join("aaa");
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = SharePayload {
        id: String::new(),
        files: vec![ShareFile {
            path: outside.to_string_lossy().into_owned(),
            filename: "secret.txt".to_string(),
            size_bytes: 4,
        }],
        text: Some("caption".to_string()),
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let payloads = take_pending_in(&d.root, &d.app_data).unwrap();

    assert_eq!(payloads.len(), 1);
    assert!(payloads[0].files.is_empty());
    assert!(outside.is_file());
}

#[test]
fn removes_unparseable_manifest() {
    let d = temp_dirs();
    let dir = d.root.join("aaa");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("manifest.json"), b"{ not json").unwrap();

    assert!(take_pending_in(&d.root, &d.app_data).unwrap().is_empty());
    assert!(!dir.exists());
}

#[test]
fn discard_removes_staged_files_after_a_drain() {
    let d = temp_dirs();
    stage(&d.root, "aaa", &["photo.jpg"], None);
    let payloads = take_pending_in(&d.root, &d.app_data).unwrap();
    let staged = PathBuf::from(&payloads[0].files[0].path);
    assert!(staged.is_file());

    discard_in(&d.root, &d.app_data, "aaa").unwrap();

    assert!(!staged.exists());
    assert!(!staged.parent().unwrap().exists());
}

#[test]
fn discard_leaves_other_shares_alone() {
    let d = temp_dirs();
    stage(&d.root, "aaa", &["photo.jpg"], None);
    stage(&d.root, "bbb", &["other.jpg"], None);
    let payloads = take_pending_in(&d.root, &d.app_data).unwrap();

    let keep = payloads
        .iter()
        .find(|p| p.id == "bbb")
        .map(|p| PathBuf::from(&p.files[0].path))
        .unwrap();

    discard_in(&d.root, &d.app_data, "aaa").unwrap();

    assert!(keep.is_file());
}

#[test]
fn discard_removes_the_drop_box_directory() {
    let d = temp_dirs();
    let dir = stage(&d.root, "aaa", &["photo.jpg"], None);

    discard_in(&d.root, &d.app_data, "aaa").unwrap();

    assert!(!dir.exists());
}

#[test]
fn discard_rejects_traversal() {
    let d = temp_dirs();
    let sibling = stage(&d.root, "bbb", &["photo.jpg"], None);

    assert!(discard_in(&d.root, &d.app_data, "../bbb").is_err());
    assert!(discard_in(&d.root, &d.app_data, "").is_err());
    assert!(sibling.exists());
}

#[test]
fn discard_of_unknown_id_succeeds() {
    let d = temp_dirs();

    discard_in(&d.root, &d.app_data, "never-existed").unwrap();
}
