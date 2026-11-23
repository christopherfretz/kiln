//! The source overlay (spec/source.md "Overlay", WP-2390): replacement
//! contents for files that exist, keyed by file identity. The front-end
//! runs over a copied tree are `kiln_driver`'s `tests/front_end_overlay.rs`.
#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use kiln_source::hash::content_hash;
use kiln_source::{FileOrigin, LazyFile, Overlay, SourceManager};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln_source-overlay-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bytes(b: &[u8]) -> Arc<[u8]> {
    Arc::from(b)
}

fn with(overlay: Overlay) -> SourceManager {
    let sm = SourceManager::new();
    sm.set_overlay(Arc::new(overlay));
    sm
}

/// An entry needs an existing regular file; the second entry for a file
/// (by another path) replaces the first.
#[test]
fn entries_name_existing_regular_files() {
    let d = TempDir::new("entries");
    let p = d.0.join("h.h");
    fs::write(&p, "disk\n").unwrap();
    let mut o = Overlay::new();
    assert!(o.is_empty());
    let missing = o.replace(&d.0.join("none.h"), bytes(b"x")).unwrap_err();
    assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    let dir = o.replace(&d.0, bytes(b"x")).unwrap_err();
    assert_eq!(dir.kind(), io::ErrorKind::InvalidInput);
    assert!(o.is_empty());
    o.replace(&p, bytes(b"one\n")).unwrap();
    o.replace(&d.0.join(".").join("h.h"), bytes(b"two\n")).unwrap();
    assert_eq!(o.len(), 1);
    assert!(o.contains(&p));
    assert!(!o.contains(&d.0.join("none.h")));
    let meta = fs::metadata(&p).unwrap();
    assert_eq!(o.bytes_of(&p, &meta).map(|b| &b[..]), Some(&b"two\n"[..]));
    // The disk is untouched.
    assert_eq!(fs::read(&p).unwrap(), b"disk\n");
}

/// A load of an overlaid file takes the overlay's bytes, through any
/// spelling, a symbolic link or a hard link: one buffer, named by the
/// path first given; a file not overlaid is read from disk.
#[cfg(unix)]
#[test]
fn identity_not_spelling() {
    let d = TempDir::new("identity");
    fs::create_dir_all(d.0.join("sub")).unwrap();
    let p = d.0.join("h.h");
    fs::write(&p, "int disk;\n").unwrap();
    std::os::unix::fs::symlink(&p, d.0.join("link.h")).unwrap();
    fs::hard_link(&p, d.0.join("hard.h")).unwrap();
    let q = d.0.join("q.h");
    fs::write(&q, "int q;\n").unwrap();
    let mut o = Overlay::new();
    // Added through the symbolic link: keyed by the file it names.
    o.replace(&d.0.join("link.h"), bytes(b"int overlay;\n")).unwrap();
    let sm = with(o);
    let first = d.0.join("sub").join("..").join("h.h");
    let id = sm.load_file(&first).unwrap();
    assert_eq!(sm.text(id), "int overlay;\n");
    assert_eq!(sm.file_origin(id), FileOrigin::Overlay);
    assert_eq!(sm.original_bytes(id).as_deref(), Some(&b"int overlay;\n"[..]));
    assert_eq!(sm.file_name(id), first.to_str().unwrap());
    for other in [p.clone(), d.0.join("link.h"), d.0.join("hard.h")] {
        assert_eq!(sm.load_regular_file(&other).unwrap(), id, "{}", other.display());
    }
    assert_eq!(sm.loaded_file(&p), Some(id));
    let qid = sm.load_file(&q).unwrap();
    assert_eq!(sm.file_origin(qid), FileOrigin::Disk);
    assert_eq!(sm.text(qid), "int q;\n");
}

/// The overlay's bytes are decoded as the same bytes on disk are:
/// invalid UTF-8 replaced and reported, a byte-order mark kept.
#[test]
fn decoding_and_load_report_as_on_disk() {
    let d = TempDir::new("decode");
    let cases: [&[u8]; 3] =
        [b"\xEF\xBB\xBFint bom;\n", b"// bad \xFF\xFE\nint x;\n", b"\xEF\xBB\xBF\xC3("];
    for (i, case) in cases.iter().enumerate() {
        let on_disk = d.0.join(format!("disk{i}.h"));
        fs::write(&on_disk, case).unwrap();
        let overlaid = d.0.join(format!("over{i}.h"));
        fs::write(&overlaid, "unrelated\n").unwrap();
        let mut o = Overlay::new();
        o.replace(&overlaid, bytes(case)).unwrap();
        let sm = with(o);
        let a = sm.load_file(&on_disk).unwrap();
        let b = sm.load_file(&overlaid).unwrap();
        assert_eq!(sm.text(a), sm.text(b), "case {i}");
        assert_eq!(sm.load_report(a), sm.load_report(b), "case {i}");
        assert_eq!(sm.original_bytes(b).as_deref(), Some(*case), "case {i}");
    }
}

/// A module file's copy of an overlaid file is not used: the overlay's
/// bytes are loaded and the copy handed back, even when its size is the
/// file's. A lazy buffer of an overlaid file is `Changed`.
#[test]
fn expected_text_and_lazy_buffers() {
    let d = TempDir::new("caches");
    let p = d.0.join("h.h");
    let text = "int old;\n";
    fs::write(&p, text).unwrap();
    let mut o = Overlay::new();
    o.replace(&p, bytes(b"int new;\n")).unwrap();
    let o = Arc::new(o);
    let sm = SourceManager::new();
    sm.set_overlay(Arc::clone(&o));
    let len = u32::try_from(text.len()).unwrap();
    let hash = content_hash(text.as_bytes());
    assert_eq!(sm.add_lazy_file("h.h", &p, len, hash, None), LazyFile::Changed);
    let (id, back) = sm.load_regular_file_expecting(&p, text.to_owned()).unwrap();
    assert_eq!(back.as_deref(), Some(text));
    assert_eq!(sm.text(id), "int new;\n");
    assert_eq!(sm.file_origin(id), FileOrigin::Overlay);
    // A size that differs takes the same route.
    let sm = SourceManager::new();
    sm.set_overlay(o);
    let (id, back) = sm.load_regular_file_expecting(&p, "int older;\n".to_owned()).unwrap();
    assert_eq!(back.as_deref(), Some("int older;\n"));
    assert_eq!(sm.text(id), "int new;\n");
    // Without the overlay the copy is used as before.
    let sm = SourceManager::new();
    let (id, back) = sm.load_regular_file_expecting(&p, text.to_owned()).unwrap();
    assert_eq!((back, sm.file_origin(id)), (None, FileOrigin::Disk));
}

/// An empty overlay is none; only the first overlay counts; a manager
/// without one reads the disk.
#[test]
fn empty_overlay_is_none() {
    let d = TempDir::new("empty");
    let p = d.0.join("h.h");
    fs::write(&p, "int disk;\n").unwrap();
    let sm = with(Overlay::new());
    assert!(sm.overlay().is_none());
    let mut o = Overlay::new();
    o.replace(&p, bytes(b"int first;\n")).unwrap();
    sm.set_overlay(Arc::new(o));
    let mut o2 = Overlay::new();
    o2.replace(&p, bytes(b"int second;\n")).unwrap();
    sm.set_overlay(Arc::new(o2));
    let id = sm.load_file(&p).unwrap();
    assert_eq!(sm.text(id), "int first;\n");
    let plain = SourceManager::new();
    let id = plain.load_file(&p).unwrap();
    assert_eq!((plain.text(id), plain.file_origin(id)), ("int disk;\n", FileOrigin::Disk));
}
