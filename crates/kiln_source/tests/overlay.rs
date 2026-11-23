//! The source overlay (spec/source.md "Overlay", WP-2390, WP-2391):
//! replacement contents for files that exist, keyed by file identity;
//! files it creates (keyed by their deepest existing ancestor and the path
//! below it) and files it removes. The front-end runs over a copied tree
//! are `kiln_driver`'s `tests/front_end_overlay.rs`; header search's view
//! is `kiln_pp`'s `tests/overlay_fs.rs`.
#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use kiln_source::hash::content_hash;
use kiln_source::{DirCache, EntryState, FileOrigin, LazyFile, Overlay, Resolved, SourceManager};

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
    assert_eq!(o.bytes_of(&p, &meta).and_then(Result::ok).as_deref(), Some(&b"two\n"[..]));
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

fn kind(r: io::Result<()>) -> Option<io::ErrorKind> {
    r.err().map(|e| e.kind())
}

/// Stage 2: the last call for a file wins. Removed then created is
/// replaced; created then removed is nothing; `create` is strict and
/// `write` replaces or creates; the refusals of `remove` (a symbolic
/// link, other hard links, not a regular file, nothing there) and of
/// `create` (an ancestor that is a file, on disk or created; `..` below
/// the deepest existing directory; a directory or a dangling link there;
/// a created path that is an ancestor of another), one assertion each.
#[cfg(unix)]
#[test]
fn create_remove_write_and_refusals() {
    use io::ErrorKind::{AlreadyExists, InvalidInput, NotFound};
    let tmp = TempDir::new("states");
    let p = tmp.0.join("h.h");
    fs::write(&p, "disk\n").unwrap();
    fs::create_dir_all(tmp.0.join("sub")).unwrap();
    let mut o = Overlay::new();
    // Removed, then created: replaced (the same identity).
    o.remove(&p).unwrap();
    assert_eq!(o.state(&p), Some(EntryState::Removed));
    assert_eq!(kind(o.remove(&p)), Some(NotFound));
    o.create(&p, bytes(b"again\n")).unwrap();
    assert_eq!(o.state(&p), Some(EntryState::Replaced));
    // Replaced, then removed: removed.
    o.remove(&tmp.0.join("sub/../h.h")).unwrap();
    assert_eq!(o.state(&p), Some(EntryState::Removed));
    // Created (two levels of new directories), then removed: nothing.
    let n = tmp.0.join("new/deeper/n.h");
    o.create(&n, bytes(b"int n;\n")).unwrap();
    assert_eq!(o.state(&n), Some(EntryState::Created));
    assert!(o.contains(&tmp.0.join("new/../new/deeper/n.h")));
    assert_eq!(o.created_paths(), [n.as_path()]);
    assert_eq!(kind(o.create(&n, bytes(b""))), Some(AlreadyExists));
    o.replace(&n, bytes(b"int n2;\n")).unwrap();
    o.remove(&n).unwrap();
    assert_eq!(o.state(&n), None);
    assert!(!o.creates_files());
    assert_eq!(o.len(), 1);
    assert_eq!(kind(o.remove(&n)), Some(NotFound));
    // `create` over a file on disk; `write` over it and over nothing.
    let q = tmp.0.join("q.h");
    fs::write(&q, "q\n").unwrap();
    assert_eq!(kind(o.create(&q, bytes(b"x"))), Some(AlreadyExists));
    o.write(&q, bytes(b"q2\n")).unwrap();
    assert_eq!(o.state(&q), Some(EntryState::Replaced));
    o.write(&tmp.0.join("w.h"), bytes(b"w\n")).unwrap();
    assert_eq!(o.state(&tmp.0.join("w.h")), Some(EntryState::Created));
    o.write(&tmp.0.join("w.h"), bytes(b"w2\n")).unwrap();
    assert_eq!(o.state(&tmp.0.join("w.h")), Some(EntryState::Created));

    // `remove`'s refusals.
    std::os::unix::fs::symlink("q.h", tmp.0.join("link.h")).unwrap();
    assert_eq!(kind(o.remove(&tmp.0.join("link.h"))), Some(InvalidInput));
    let hard = tmp.0.join("hard.h");
    fs::write(&hard, "h\n").unwrap();
    fs::hard_link(&hard, tmp.0.join("hard2.h")).unwrap();
    assert_eq!(kind(o.remove(&hard)), Some(InvalidInput));
    assert_eq!(kind(o.remove(&tmp.0.join("sub"))), Some(InvalidInput));
    assert_eq!(kind(o.remove(&tmp.0.join("none.h"))), Some(NotFound));

    // `create`'s refusals.
    assert_eq!(kind(o.create(&tmp.0.join("q.h/x.h"), bytes(b""))), Some(InvalidInput));
    assert_eq!(kind(o.create(&tmp.0.join("w.h/x.h"), bytes(b""))), Some(InvalidInput));
    assert_eq!(kind(o.create(&tmp.0.join("gone/../x.h"), bytes(b""))), Some(InvalidInput));
    assert_eq!(kind(o.create(&tmp.0.join("sub"), bytes(b""))), Some(InvalidInput));
    std::os::unix::fs::symlink("nowhere", tmp.0.join("dangling")).unwrap();
    assert_eq!(kind(o.create(&tmp.0.join("dangling"), bytes(b""))), Some(InvalidInput));
    assert_eq!(kind(o.create(&tmp.0.join("dangling/x.h"), bytes(b""))), Some(InvalidInput));
    o.create(&tmp.0.join("v/a.h"), bytes(b"")).unwrap();
    assert_eq!(kind(o.create(&tmp.0.join("v"), bytes(b""))), Some(InvalidInput));
    assert_eq!(kind(o.create(&tmp.0.join("v/a.h/b.h"), bytes(b""))), Some(InvalidInput));
    // Nothing was written.
    assert!(
        !tmp.0.join("new").exists() && !tmp.0.join("v").exists() && !tmp.0.join("w.h").exists()
    );
    assert_eq!(fs::read(&p).unwrap(), b"disk\n");
}

/// A created file loads from the overlay's bytes as a file on disk with
/// them would (`FileOrigin::Overlay`, named by the path first given):
/// two spellings, a `..` out of a created directory and a symbolic link
/// to its existing directory are one buffer; a created directory is a
/// directory. A removed file is not found by any spelling, and neither a
/// lazy buffer nor a module file's copy of it is used.
#[cfg(unix)]
#[test]
fn created_and_removed_files_load_as_the_edited_tree() {
    let d = TempDir::new("loads");
    fs::create_dir_all(d.0.join("inc")).unwrap();
    std::os::unix::fs::symlink("inc", d.0.join("alias")).unwrap();
    let gone = d.0.join("inc/gone.h");
    let text = "int gone;\n";
    fs::write(&gone, text).unwrap();
    let mut o = Overlay::new();
    o.create(&d.0.join("inc/new.h"), bytes(b"int created;\n")).unwrap();
    o.create(&d.0.join("fresh/dir/f.h"), bytes(b"\xEF\xBB\xBFint f;\n")).unwrap();
    o.remove(&gone).unwrap();
    let sm = with(o);
    let first = d.0.join("alias/new.h");
    let id = sm.load_file(&first).unwrap();
    assert_eq!(sm.text(id), "int created;\n");
    assert_eq!(sm.file_origin(id), FileOrigin::Overlay);
    assert_eq!(sm.file_name(id), first.to_str().unwrap());
    for other in ["inc/new.h", "inc/../inc/new.h", "fresh/dir/../../inc/new.h"] {
        assert_eq!(sm.load_regular_file(&d.0.join(other)).unwrap(), id, "{other}");
    }
    assert_eq!(sm.loaded_file(&d.0.join("inc/new.h")), Some(id));
    let f = sm.load_file(&d.0.join("fresh/./dir/f.h")).unwrap();
    assert_eq!(sm.original_bytes(f).as_deref(), Some(&b"\xEF\xBB\xBFint f;\n"[..]));
    let dir = sm.load_file(&d.0.join("fresh/dir")).unwrap_err();
    assert!(dir.to_string().contains("directory"), "{dir}");
    assert!(sm.load_file(&d.0.join("fresh/none.h")).is_err());
    // Out of created directories with `..`: the disk's file, one buffer.
    fs::write(d.0.join("inc/keep.h"), "int keep;\n").unwrap();
    let keep = sm.load_file(&d.0.join("fresh/dir/../../inc/keep.h")).unwrap();
    assert_eq!((sm.text(keep), sm.file_origin(keep)), ("int keep;\n", FileOrigin::Disk));
    assert_eq!(sm.load_file(&d.0.join("inc/keep.h")).unwrap(), keep);
    assert!(sm.is_regular_file(&d.0.join("fresh/dir/f.h")));
    assert!(!sm.is_regular_file(&d.0.join("fresh/dir")));
    // Removed: by every spelling.
    for spelling in ["inc/gone.h", "alias/gone.h", "inc/../inc/gone.h"] {
        assert!(sm.load_file(&d.0.join(spelling)).is_err(), "{spelling}");
    }
    assert!(!sm.is_regular_file(&gone));
    assert_eq!(sm.loaded_file(&gone), None);
    let len = u32::try_from(text.len()).unwrap();
    assert_eq!(
        sm.add_lazy_file("g", &gone, len, content_hash(text.as_bytes()), None),
        LazyFile::Missing
    );
    assert!(sm.load_regular_file_expecting(&gone, text.to_owned()).is_err());
}

/// `Overlay::resolve`: a `..` after a created directory leaves it
/// lexically and reaches the disk again; a created directory has one
/// identity for every spelling and lists its created entries; a miss
/// whose last component no created path has is answered at once.
#[cfg(unix)]
#[test]
fn resolve_paths() {
    let d = TempDir::new("resolve");
    fs::write(d.0.join("x.h"), "x\n").unwrap();
    let mut o = Overlay::new();
    o.create(&d.0.join("newdir/a.h"), bytes(b"a")).unwrap();
    o.create(&d.0.join("newdir/sub/b.h"), bytes(b"b")).unwrap();
    let mut cache = DirCache::default();
    let r = |o: &Overlay, p: &str, c: &mut DirCache| format!("{:?}", o.resolve(&d.0.join(p), c));
    assert!(r(&o, "newdir/../x.h", &mut cache).starts_with("Disk"));
    assert!(r(&o, "newdir/sub/../a.h", &mut cache).starts_with("Created"));
    assert_eq!(r(&o, "newdir/a.h/../a.h", &mut cache), "Missing");
    assert_eq!(r(&o, "nodir/../x.h", &mut cache), "Missing");
    assert_eq!(r(&o, "newdir/zz.h", &mut cache), "Missing");
    let ident = |p: &str, c: &mut DirCache| match o.resolve(&d.0.join(p), c) {
        Resolved::CreatedDir(dir) => {
            let mut names: Vec<_> = dir.names().map(|n| n.to_string_lossy().into_owned()).collect();
            names.sort();
            (dir.identity(), names)
        }
        other => panic!("{p}: {other:?}"),
    };
    let (a, names) = ident("newdir", &mut cache);
    assert_eq!(names, ["a.h", "sub"]);
    assert_eq!(ident("./newdir/sub/..", &mut cache).0, a);
    assert_ne!(ident("newdir/sub", &mut cache).0, a);
    let mut top: Vec<_> =
        o.created_in(&d.0, &mut cache).into_iter().map(ToOwned::to_owned).collect();
    top.sort();
    assert_eq!(top, ["newdir"]);
}

/// Lazy contents: the checks and the key are taken when the entry is
/// added, the bytes on the first load (once per overlay, whichever source
/// manager loads first); a failure is kept, the file is not found, and
/// each source manager that loads it notes it once.
#[test]
fn lazy_contents() {
    use std::sync::atomic::{AtomicU32, Ordering};
    let t = TempDir::new("lazy");
    let (old, new, bad) = (t.0.join("old.h"), t.0.join("new.h"), t.0.join("bad.h"));
    fs::write(&old, "disk\n").unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    let mut o = Overlay::new();
    let c = Arc::clone(&calls);
    let loader: kiln_source::Loader = Box::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(bytes(b"loaded\n"))
    });
    o.replace_with(&old, loader).unwrap();
    o.create_with(&new, Box::new(|| Ok(bytes(b"made\n")))).unwrap();
    o.create_with(&bad, Box::new(|| Err(io::Error::other("no")))).unwrap();
    assert_eq!(
        kind(o.create_with(&new, Box::new(|| Ok(bytes(b""))))),
        Some(io::ErrorKind::AlreadyExists)
    );
    assert_eq!(
        kind(o.replace_with(&t.0.join("none/x.h"), Box::new(|| Ok(bytes(b""))))),
        Some(io::ErrorKind::NotFound)
    );
    assert_eq!(o.state(&new), Some(EntryState::Created));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let o = Arc::new(o);
    for _ in 0..2 {
        let sm = SourceManager::new();
        sm.set_overlay(Arc::clone(&o));
        let f = sm.load_file(&old).unwrap();
        assert_eq!(sm.text(f), "loaded\n");
        let f = sm.load_file(&new).unwrap();
        assert_eq!(sm.text(f), "made\n");
        assert!(sm.load_file(&bad).is_err());
        assert!(sm.load_file(&bad).is_err());
        assert_eq!(sm.overlay_failures().list(), [(bad.clone(), "no".to_owned())]);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(o.load_failures(), [(bad.clone(), "no".to_owned())]);
    let meta = fs::metadata(&old).unwrap();
    assert_eq!(o.bytes_of(&old, &meta), Some(Ok(bytes(b"loaded\n"))));
    assert!(format!("{o:?}").contains("lazy, failed"));
}
