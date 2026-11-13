//! Module files (spec/modules.md §11): the *source capsule* format
//! (kind 1) and the sectioned format that extends it (kind 2, §11.11).
//!
//! A capsule records the inputs of a module interface unit's
//! preprocessing (its files, every include and embed lookup with its
//! result, the predefines and a flags fingerprint), so that an importer
//! rebuilds the unit exactly as the interface compile read it. This module
//! holds the data model, the writer and the reader; recording and replay
//! are in `modules.rs`.
//!
//! A kind-2 file is a capsule followed by a list of *sections*: opaque,
//! tagged byte blobs whose contents the crates above the preprocessor
//! define (the carried constant results of `kiln_sema::bmi_ast`, tag
//! [`SECTION_CONSTS`]). This module frames and checks them; it never
//! interprets their bytes.
//!
//! Reading is total: every malformed input is a [`BmiError`], never a
//! panic, and no allocation is sized by an unchecked length.

use std::fmt;
use std::io::{self, Write as _};
use std::path::Path;

use crate::include::IncludeKind;

/// The first 8 bytes of every kiln module file.
pub const MAGIC: [u8; 8] = *b"KILNBMI\0";
/// The first 4 bytes of a clang module file (an AST file).
pub const CLANG_MAGIC: [u8; 4] = *b"CPCH";
/// The layout version this reader accepts and this writer produces.
pub const FORMAT_VERSION: u32 = 1;
/// The kind byte of a source capsule.
pub const KIND_CAPSULE: u8 = 1;
/// The kind byte of a capsule followed by sections (spec/modules.md
/// §11.11): written when [`Capsule::sections`] is not empty.
pub const KIND_SECTIONS: u8 = 2;
/// The section of carried constant-evaluation results (spec/modules.md
/// §11.11, "Constant state"; encoded by `kiln_sema::bmi_ast`).
pub const SECTION_CONSTS: u32 = 1;
/// The declaration and type image (spec/modules.md §11.11 "Part 2a";
/// encoded by `kiln_sema::bmi_image`).
pub const SECTION_IMAGE: u32 = 2;
/// The preprocessor-state record (spec/modules.md §11.13; encoded by
/// [`crate::state`]).
pub const SECTION_PP: u32 = 3;
/// The whole-front-end snapshot of a precompiled header (WP-1383, an
/// experimental prototype: `-fkiln-pch-snapshot`, spec/snap.md
/// "Whole-compiler restore order"). Valid only for the same build.
pub const SECTION_SNAPSHOT: u32 = 4;
/// The highest section tag this reader knows. Tags above it are reserved
/// for later parts of the serialised AST (§11.11, "Sections").
pub const SECTION_MAX: u32 = SECTION_SNAPSHOT;
/// The includer index of the predefines buffer (`<built-in>`).
pub const INCLUDER_BUILTIN: u32 = u32::MAX;
/// The includer index of a request without an includer.
pub const INCLUDER_NONE: u32 = u32::MAX - 1;

/// The version of kiln that writes and accepts module files.
pub const KILN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The git sha of this kiln build, when the build recorded one
/// (`KILN_GIT_SHA` at compile time); empty otherwise.
#[must_use]
pub fn kiln_sha() -> &'static str {
    option_env!("KILN_GIT_SHA").unwrap_or("")
}

/// Which kind of importable unit a capsule holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitKind {
    /// `export module M;`.
    Interface,
    /// `export module M:P;`.
    InterfacePartition,
    /// `module M:P;`.
    ImplementationPartition,
}

impl UnitKind {
    fn code(self) -> u8 {
        match self {
            UnitKind::Interface => 1,
            UnitKind::InterfacePartition => 2,
            UnitKind::ImplementationPartition => 3,
        }
    }

    fn from_code(c: u8) -> Option<UnitKind> {
        Some(match c {
            1 => UnitKind::Interface,
            2 => UnitKind::InterfacePartition,
            3 => UnitKind::ImplementationPartition,
            _ => return None,
        })
    }
}

/// One file the unit read.
#[derive(Clone, PartialEq, Eq)]
pub struct CapsuleFile {
    /// The path it was first loaded by, as the compile spelled it.
    pub path: String,
    /// Its text (after UTF-8 decoding, as the source manager held it).
    pub text: String,
}

impl fmt::Debug for CapsuleFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapsuleFile")
            .field("path", &self.path)
            .field("len", &self.text.len())
            .finish()
    }
}

/// A found file in the include log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncludeHit {
    /// Index into [`Capsule::files`].
    pub file: u32,
    /// [`crate::ResolvedInclude::lookup_index`].
    pub lookup_index: Option<u32>,
    /// [`crate::ResolvedInclude::system`].
    pub system: bool,
    /// [`crate::ResolvedInclude::extern_c`].
    pub extern_c: bool,
    /// [`crate::ResolvedInclude::path`].
    pub path: Option<String>,
}

/// The lookup part of an include-log entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IncludeKey {
    /// The includer's file index ([`INCLUDER_BUILTIN`], [`INCLUDER_NONE`]).
    pub includer: u32,
    /// The header name as written.
    pub name: String,
    /// `<...>`.
    pub angled: bool,
    /// The construct.
    pub kind: IncludeKind,
    /// [`crate::IncludeRequest::lookup_from`].
    pub lookup_from: Option<u32>,
}

/// One include lookup and its result (`None`: not found).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncludeEntry {
    /// What was asked.
    pub key: IncludeKey,
    /// What was found.
    pub hit: Option<IncludeHit>,
}

/// The lookup part of an embed-log entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EmbedKey {
    /// The includer's file index.
    pub includer: u32,
    /// The resource name.
    pub name: String,
    /// `<...>`.
    pub angled: bool,
    /// `limit(N)`.
    pub limit: Option<u64>,
    /// `clang::offset(N)`.
    pub offset: Option<u64>,
    /// `__has_embed`.
    pub query_only: bool,
}

/// One embed lookup and its bytes (`None`: not found).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedEntry {
    /// What was asked.
    pub key: EmbedKey,
    /// The bytes, already cut to `offset`/`limit`.
    pub bytes: Option<Vec<u8>>,
}

/// A source-capsule module file (spec/modules.md §11.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capsule {
    /// The kiln version that wrote it.
    pub version: String,
    /// The git sha of that kiln, or empty.
    pub sha: String,
    /// The module key: `M` or `M:P`.
    pub module: String,
    /// The unit's kind.
    pub kind: UnitKind,
    /// The `<built-in>` buffer of the compile that wrote it.
    pub predefines: String,
    /// The flags fingerprint: (key, effective flag spelling) pairs.
    pub fingerprint: Vec<(String, String)>,
    /// The unit's named-module imports and the path each was found at.
    pub imports: Vec<(String, String)>,
    /// Every file the unit read, its main file first.
    pub files: Vec<CapsuleFile>,
    /// The include log.
    pub includes: Vec<IncludeEntry>,
    /// The embed log.
    pub embeds: Vec<EmbedEntry>,
    /// Kind-2 sections, by strictly increasing tag; empty for a kind-1
    /// file (spec/modules.md §11.11).
    pub sections: Vec<Section>,
}

/// The carried-constants section of an imported kind-2 module file and
/// the file's path (for the corrupt-file diagnostic), handed to the
/// parser with the unit's [`crate::Marker::UnitBegin`].
#[derive(Debug, PartialEq, Eq)]
pub struct CarriedSection {
    /// The module file's path, as shown in diagnostics.
    pub path: String,
    /// The [`SECTION_CONSTS`] bytes.
    pub bytes: Vec<u8>,
}

/// The declaration image of an imported kind-2 module file, handed to the
/// parser with the unit's [`crate::Marker::UnitBegin`] in place of the
/// unit's tokens (spec/modules.md §11.11 "Part 2a").
#[derive(Debug, PartialEq, Eq)]
pub struct ImageSection {
    /// The module file's path, as shown in diagnostics.
    pub path: String,
    /// The [`SECTION_IMAGE`] bytes (moved out of the capsule).
    pub bytes: Vec<u8>,
    /// The buffers of the image's file table (§11.11 "File table"), in
    /// table order: index 0 is the unit's main file, loaded as the replay
    /// loads it (§11.2); the others are lazy buffers, in-memory buffers,
    /// or files the importer had loaded with the recorded text.
    pub files: Vec<kiln_source::FileId>,
}

/// One kind-2 section: a tag and its bytes, which this crate does not
/// interpret (spec/modules.md §11.11).
#[derive(Clone, PartialEq, Eq)]
pub struct Section {
    /// What the bytes hold ([`SECTION_CONSTS`]).
    pub tag: u32,
    /// The section's contents.
    pub bytes: Vec<u8>,
}

impl fmt::Debug for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Section({}, {} bytes)", self.tag, self.bytes.len())
    }
}

impl Capsule {
    /// The kind byte this capsule is written with: [`KIND_SECTIONS`] when
    /// it has sections, else [`KIND_CAPSULE`].
    #[must_use]
    pub fn file_kind(&self) -> u8 {
        if self.sections.is_empty() { KIND_CAPSULE } else { KIND_SECTIONS }
    }

    /// The bytes of section `tag`, if present.
    #[must_use]
    pub fn section(&self, tag: u32) -> Option<&[u8]> {
        self.sections.iter().find(|s| s.tag == tag).map(|s| s.bytes.as_slice())
    }
}

/// The fixed fields at the start of a module file, readable without the
/// rest ([`peek`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The kiln version that wrote it.
    pub version: String,
    /// Its git sha, or empty.
    pub sha: String,
    /// The module key.
    pub module: String,
    /// The unit's kind.
    pub kind: UnitKind,
}

/// Why a module file cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BmiError {
    /// Not a kiln module file; `clang` when it has clang's AST-file magic.
    NotKiln {
        /// It starts with `CPCH`.
        clang: bool,
    },
    /// Another layout version.
    Format(u32),
    /// Malformed: truncated, a bad checksum, a bad length or index.
    Corrupt(String),
}

impl fmt::Display for BmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BmiError::NotKiln { clang: true } => f.write_str("built by clang"),
            BmiError::NotKiln { clang: false } => f.write_str("not a kiln module file"),
            BmiError::Format(v) => write!(f, "format version {v}, expected {FORMAT_VERSION}"),
            BmiError::Corrupt(why) => f.write_str(why),
        }
    }
}

/// The checksum: FNV-1a 64. Each step is a bijection of the state for a
/// fixed input byte, so changing any one byte changes the result.
#[must_use]
pub fn checksum(bytes: &[u8]) -> u64 {
    fnv(FNV_OFFSET, bytes)
}

/// The checksum of a precompiled header's section block and image
/// ([`write_sections`]; spec/modules.md §11.13 "In a precompiled
/// header"): word-wise, four independent lanes of 8-byte words, so it
/// runs at memory speed where FNV-1a 64 ([`checksum`]) is one multiply
/// per byte (96 ms against about 5 ms over a 36 MB snapshot section).
/// Each lane step `l = rotl((l ^ w) * K, 29)` is a bijection of the lane
/// for a fixed word and of the word for a fixed lane, the bytes after
/// the last whole 32-byte group go through FNV-1a, and the lanes are
/// folded in one at a time by the same kind of step, so changing any one
/// byte (or the length) changes the result, as with [`checksum`].
/// Module files keep [`checksum`]: changing it would change their format.
#[must_use]
pub fn block_checksum(bytes: &[u8]) -> u64 {
    let mut c = BlockChecksum::new();
    c.update(bytes);
    c.finish()
}

/// [`block_checksum`] of bytes given a piece at a time (WP-1393: a PCH
/// consumer that streams its snapshot from the file checks the block as
/// it reads it): after [`update`](Self::update)s with pieces whose
/// concatenation is `b`, [`finish`](Self::finish) is `block_checksum(b)`.
#[derive(Clone, Debug)]
pub struct BlockChecksum {
    lanes: [u64; 4],
    /// The bytes after the last whole 32-byte group so far.
    carry: [u8; 32],
    carried: usize,
    len: u64,
}

impl Default for BlockChecksum {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockChecksum {
    const K: u64 = 0x9e37_79b9_7f4a_7c15;

    /// The checksum of no bytes so far.
    #[must_use]
    pub fn new() -> Self {
        BlockChecksum {
            lanes: [
                0x243f_6a88_85a3_08d3,
                0x1319_8a2e_0370_7344,
                0xa409_3822_299f_31d0,
                0x082e_fa98_ec4e_6c89,
            ],
            carry: [0; 32],
            carried: 0,
            len: 0,
        }
    }

    #[inline]
    fn step(l: u64, w: u64) -> u64 {
        (l ^ w).wrapping_mul(Self::K).rotate_left(29)
    }

    #[inline]
    fn group(lanes: &mut [u64; 4], g: &[u8; 32]) {
        let (words, _) = g.as_chunks::<8>();
        for (l, w) in lanes.iter_mut().zip(words) {
            *l = Self::step(*l, u64::from_le_bytes(*w));
        }
    }

    /// Adds the next `bytes`.
    pub fn update(&mut self, mut bytes: &[u8]) {
        self.len += bytes.len() as u64;
        if self.carried > 0 {
            let k = (32 - self.carried).min(bytes.len());
            self.carry[self.carried..self.carried + k].copy_from_slice(&bytes[..k]);
            self.carried += k;
            bytes = &bytes[k..];
            if self.carried < 32 {
                return;
            }
            let g = self.carry;
            Self::group(&mut self.lanes, &g);
            self.carried = 0;
        }
        let (groups, rest) = bytes.as_chunks::<32>();
        for g in groups {
            Self::group(&mut self.lanes, g);
        }
        self.carry[..rest.len()].copy_from_slice(rest);
        self.carried = rest.len();
    }

    /// The checksum of every byte added.
    #[must_use]
    pub fn finish(&self) -> u64 {
        let mut h = fnv(FNV_OFFSET ^ self.len, &self.carry[..self.carried]);
        for l in self.lanes {
            h = Self::step(h, l);
        }
        h ^ (h >> 31)
    }
}

/// True if `bytes` starts with the kiln magic.
#[must_use]
pub fn is_module_file(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

// --- writer -----------------------------------------------------------

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    fn len(&mut self, n: usize) {
        // Lengths beyond 4 GiB cannot occur: the source manager's
        // location space is smaller.
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
    fn bytes(&mut self, b: &[u8]) {
        self.len(b.len());
        self.0.extend_from_slice(b);
    }
    fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
    fn opt_u32(&mut self, v: Option<u32>) {
        match v {
            Some(v) => {
                self.u8(1);
                self.u32(v);
            }
            None => self.u8(0),
        }
    }
    fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            Some(v) => {
                self.u8(1);
                self.u64(v);
            }
            None => self.u8(0),
        }
    }
    fn opt_str(&mut self, v: Option<&str>) {
        match v {
            Some(v) => {
                self.u8(1);
                self.str(v);
            }
            None => self.u8(0),
        }
    }
    fn pairs(&mut self, v: &[(String, String)]) {
        self.len(v.len());
        for (a, b) in v {
            self.str(a);
            self.str(b);
        }
    }
}

fn kind_code(k: IncludeKind) -> u8 {
    match k {
        IncludeKind::Include => 0,
        IncludeKind::IncludeNext => 1,
        IncludeKind::Import => 2,
        IncludeKind::HasInclude => 3,
        IncludeKind::HasIncludeNext => 4,
    }
}

fn kind_from_code(c: u8) -> Option<IncludeKind> {
    Some(match c {
        0 => IncludeKind::Include,
        1 => IncludeKind::IncludeNext,
        2 => IncludeKind::Import,
        3 => IncludeKind::HasInclude,
        4 => IncludeKind::HasIncludeNext,
        _ => return None,
    })
}

/// What a file holds, from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sniffed {
    /// A kiln module file (it starts with [`MAGIC`]).
    Kiln,
    /// A clang PCM (it starts with [`CLANG_MAGIC`]).
    Clang,
    /// Anything else: a source.
    Source,
    /// The file cannot be read.
    Missing,
}

/// What the preprocessor reads from an image section's header
/// ([`image_header`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageHeader<'a> {
    /// The unit's main file path.
    pub main: &'a str,
    /// S6 (spec/modules.md §11.11 "Imports, partitions and
    /// using-declarations"): the main-file offset just past the unit's
    /// last top-level module or import directive, replayed before the
    /// image is built; `None` for a unit that replays nothing.
    pub preamble: Option<u32>,
    /// The gated names.
    pub gated: Vec<&'a str>,
    /// S7: the file table (index 0: the main file).
    pub files: Vec<ImageFile<'a>>,
}

/// One entry of an image's file table (spec/modules.md §11.11 "File
/// table").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageFile<'a> {
    /// The buffer's name, as diagnostics print it.
    pub name: &'a str,
    /// Its absolute path when it was built (empty for an in-memory buffer).
    pub path: &'a str,
    /// The decoded text's length.
    pub len: u32,
    /// The text's [`checksum`] (`kiln_source::hash::content_hash`).
    pub hash: u64,
    /// It was entered as a system header.
    pub system: bool,
    /// An in-memory buffer's text.
    pub text: Option<&'a str>,
}

/// The main file path, the preamble and the gated names an image section
/// lists up front (spec/modules.md §11.11 "Part 2a", "Encoding"): the
/// names of its declarations at global namespace scope, which the
/// importer's preprocessor checks against the names it has seen there
/// before it uses the image; `None` when the header does not decode (the
/// reader then reports the file as corrupt).
pub fn image_header(bytes: &[u8]) -> Option<ImageHeader<'_>> {
    let mut at = 0usize;
    let u32_at = |at: &mut usize| -> Option<u32> {
        let b = bytes.get(*at..at.checked_add(4)?)?;
        *at += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let str_at = |at: &mut usize| -> Option<&str> {
        let n = u32_at(at)? as usize;
        let b = bytes.get(*at..at.checked_add(n)?)?;
        *at += n;
        std::str::from_utf8(b).ok()
    };
    let _version = u32_at(&mut at)?;
    let main = str_at(&mut at)?;
    let _module = str_at(&mut at)?;
    let _kw = u32_at(&mut at)?;
    let _name_loc = u32_at(&mut at)?;
    let preamble = Some(u32_at(&mut at)?).filter(|&p| p != u32::MAX);
    let n = u32_at(&mut at)? as usize;
    if n.saturating_mul(4) > bytes.len() - at {
        return None;
    }
    let names = (0..n).map(|_| str_at(&mut at)).collect::<Option<Vec<_>>>()?;
    let g = u32_at(&mut at)? as usize;
    if g.saturating_mul(4) > bytes.len() - at {
        return None;
    }
    let gated = (0..g)
        .map(|_| names.get(u32_at(&mut at)? as usize).copied())
        .collect::<Option<Vec<_>>>()?;
    let nf = u32_at(&mut at)? as usize;
    if nf.saturating_mul(21) > bytes.len() - at {
        return None;
    }
    let mut files = Vec::with_capacity(nf);
    for _ in 0..nf {
        let (name, path, len) = (str_at(&mut at)?, str_at(&mut at)?, u32_at(&mut at)?);
        let lo = u64::from(u32_at(&mut at)?);
        let hash = lo | (u64::from(u32_at(&mut at)?) << 32);
        let flags = *bytes.get(at)?;
        at += 1;
        let text = if flags & 2 != 0 { Some(str_at(&mut at)?) } else { None };
        if flags & 4 != 0 {
            // The buffer's line directives: `(u32, u32, u8 named, name)`,
            // read by the image's own decoder.
            for _ in 0..u32_at(&mut at)? {
                let _ = (u32_at(&mut at)?, u32_at(&mut at)?);
                let has_name = *bytes.get(at)?;
                at += 1;
                if has_name != 0 {
                    str_at(&mut at)?;
                }
            }
        }
        files.push(ImageFile { name, path, len, hash, system: flags & 1 != 0, text });
    }
    Some(ImageHeader { main, preamble, gated, files })
}

/// Sniffs the file at `path`, reading at most 8 bytes.
pub fn sniff(path: &Path) -> Sniffed {
    use std::io::Read as _;
    let Ok(f) = std::fs::File::open(path) else { return Sniffed::Missing };
    let mut head = Vec::with_capacity(8);
    if f.take(8).read_to_end(&mut head).is_err() {
        return Sniffed::Missing;
    }
    if is_module_file(&head) {
        Sniffed::Kiln
    } else if head.starts_with(&CLANG_MAGIC) {
        Sniffed::Clang
    } else {
        Sniffed::Source
    }
}

/// The bytes of `c` (spec/modules.md §11.3). Deterministic: the same
/// capsule always gives the same bytes.
#[must_use]
pub fn write(c: &Capsule) -> Vec<u8> {
    let size: usize = c.files.iter().map(|f| f.path.len() + f.text.len() + 8).sum();
    let mut w = W(Vec::with_capacity(size + c.predefines.len() + 4096));
    w.0.extend_from_slice(&MAGIC);
    w.u32(FORMAT_VERSION);
    w.u8(c.file_kind());
    w.str(&c.version);
    w.str(&c.sha);
    w.str(&c.module);
    w.u8(c.kind.code());
    w.str(&c.predefines);
    w.pairs(&c.fingerprint);
    w.pairs(&c.imports);
    w.len(c.files.len());
    for f in &c.files {
        w.str(&f.path);
        w.str(&f.text);
    }
    w.len(c.includes.len());
    for e in &c.includes {
        w.u32(e.key.includer);
        w.str(&e.key.name);
        w.bool(e.key.angled);
        w.u8(kind_code(e.key.kind));
        w.opt_u32(e.key.lookup_from);
        match &e.hit {
            Some(h) => {
                w.u8(1);
                w.u32(h.file);
                w.opt_u32(h.lookup_index);
                w.bool(h.system);
                w.bool(h.extern_c);
                w.opt_str(h.path.as_deref());
            }
            None => w.u8(0),
        }
    }
    w.len(c.embeds.len());
    for e in &c.embeds {
        w.u32(e.key.includer);
        w.str(&e.key.name);
        w.bool(e.key.angled);
        w.opt_u64(e.key.limit);
        w.opt_u64(e.key.offset);
        w.bool(e.key.query_only);
        match &e.bytes {
            Some(b) => {
                w.u8(1);
                w.bytes(b);
            }
            None => w.u8(0),
        }
    }
    if !c.sections.is_empty() {
        section_block(&mut w, &c.sections);
    }
    let sum = checksum(&w.0);
    w.u64(sum);
    w.0
}

/// The section block of a kind-2 file: the count, then each section's
/// tag and bytes, by increasing tag.
fn section_block(w: &mut W, sections: &[Section]) {
    let mut sorted: Vec<&Section> = sections.iter().collect();
    sorted.sort_by_key(|s| s.tag);
    w.len(sorted.len());
    for s in sorted {
        w.u32(s.tag);
        w.bytes(&s.bytes);
    }
}

/// The section block alone, as a precompiled header carries it after its
/// `KILNPCH` record (spec/modules.md §11.13 "In a precompiled header"):
/// the kind-2 block ([`write`]) followed by its [`block_checksum`].
#[must_use]
pub fn write_sections(sections: &[Section]) -> Vec<u8> {
    let mut w = W(Vec::new());
    section_block(&mut w, sections);
    let sum = block_checksum(&w.0);
    w.u64(sum);
    w.0
}

/// Reads a block written by [`write_sections`]. Total: every malformed
/// input is an error.
///
/// # Errors
/// [`BmiError::Corrupt`] for a truncated block, an unknown or repeated
/// tag, trailing bytes or a checksum mismatch.
pub fn read_sections(bytes: &[u8]) -> Result<Vec<Section>, BmiError> {
    Ok(section_ranges(bytes)?
        .into_iter()
        .map(|(tag, at)| Section { tag, bytes: bytes[at].to_vec() })
        .collect())
}

/// [`read_sections`] without copying (WP-1393): each section's tag and
/// where its bytes are in `bytes`, after the same checks (the checksum
/// first, over the whole block).
///
/// # Errors
/// As [`read_sections`].
pub fn section_ranges(bytes: &[u8]) -> Result<Vec<(u32, std::ops::Range<usize>)>, BmiError> {
    let Some(body_len) = bytes.len().checked_sub(8) else { return corrupt("truncated") };
    let mut t = [0u8; 8];
    t.copy_from_slice(&bytes[body_len..]);
    if u64::from_le_bytes(t) != block_checksum(&bytes[..body_len]) {
        return corrupt("checksum mismatch");
    }
    let body = &bytes[..body_len];
    section_table(body_len, |at| body.get(at..at + 4).map(|w| [w[0], w[1], w[2], w[3]]))
}

/// The section table of a block whose body (the block less its 8-byte
/// checksum) is `body_len` bytes, its words read by `word(offset)`
/// (`None` past the end): [`section_ranges`] after its checksum, for a
/// block whose checksum was checked as it was read and whose bytes stay
/// in the file (WP-1401). Reads only the count and each section's tag
/// and length.
///
/// # Errors
/// As [`read_sections`], but for the checksum.
pub fn section_table(
    body_len: usize,
    mut word: impl FnMut(usize) -> Option<[u8; 4]>,
) -> Result<Vec<(u32, std::ops::Range<usize>)>, BmiError> {
    let mut at = 0usize;
    let mut u32_at = |at: &mut usize| -> Res<u32> {
        let Some(w) = word(*at).filter(|_| *at + 4 <= body_len) else {
            return corrupt("truncated");
        };
        *at += 4;
        Ok(u32::from_le_bytes(w))
    };
    // A count or length is checked against what is left, as `R::count`.
    let n = u32_at(&mut at)? as usize;
    if (n as u64).saturating_mul(8) > (body_len - at) as u64 {
        return corrupt("length exceeds the file");
    }
    if n == 0 {
        return corrupt("a kind-2 file without sections");
    }
    let mut out: Vec<(u32, std::ops::Range<usize>)> = Vec::with_capacity(n);
    for _ in 0..n {
        let tag = u32_at(&mut at)?;
        if tag == 0 || tag > SECTION_MAX {
            return corrupt("unknown section");
        }
        if out.last().is_some_and(|s| s.0 >= tag) {
            return corrupt("sections out of order");
        }
        let len = u32_at(&mut at)? as usize;
        if len > body_len - at {
            return corrupt("length exceeds the file");
        }
        out.push((tag, at..at + len));
        at += len;
    }
    if at != body_len {
        return corrupt("trailing bytes");
    }
    Ok(out)
}

/// Reads a section block ([`section_block`]).
fn sections(r: &mut R<'_>) -> Res<Vec<Section>> {
    let n = r.count(8)?;
    if n == 0 {
        return corrupt("a kind-2 file without sections");
    }
    let mut sections: Vec<Section> = Vec::with_capacity(n);
    for _ in 0..n {
        let tag = r.u32()?;
        if tag == 0 || tag > SECTION_MAX {
            return corrupt("unknown section");
        }
        if sections.last().is_some_and(|s| s.tag >= tag) {
            return corrupt("sections out of order");
        }
        sections.push(Section { tag, bytes: r.bytes()? });
    }
    Ok(sections)
}

/// Writes `bytes` to `path` atomically: to a temporary file beside it,
/// then renamed over it, so an interrupted compile never leaves a
/// half-written module file. A path that exists and is not a regular
/// file (`-o /dev/null`, a FIFO) is written in place, as clang does:
/// renaming over it would replace the device with a regular file.
///
/// # Errors
/// The I/O error of creating, writing or renaming the file.
pub fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if std::fs::metadata(path).is_ok_and(|m| !m.is_file()) {
        let mut f = std::fs::OpenOptions::new().write(true).open(path)?;
        return f.write_all(bytes).and_then(|()| f.flush());
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{}", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    let res = std::fs::File::create(&tmp)
        .and_then(|mut f| f.write_all(bytes).and_then(|()| f.flush()))
        .and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

// --- reader -----------------------------------------------------------

/// The streaming reader: every value is read from `src` as it is decoded,
/// so a file's text is read straight into the buffer that keeps it and no
/// whole-file copy is ever held (spec/modules.md §11.8: freeing one large
/// buffer raises glibc's mmap threshold and inflates the importer's peak
/// RSS by about the module file's size). `left` counts the body bytes not
/// yet read; every length is checked against it before anything is
/// allocated. `sum` is the running checksum of everything read.
struct R<'a> {
    /// The kind byte (checked by [`prefix`]).
    kind: u8,
    src: &'a mut dyn io::Read,
    left: u64,
    sum: u64,
    /// Whether `sum` is kept ([`read_sections`] checks its block first,
    /// with [`block_checksum`]).
    track: bool,
}

type Res<T> = Result<T, BmiError>;

fn corrupt<T>(why: &str) -> Res<T> {
    Err(BmiError::Corrupt(why.to_owned()))
}

use kiln_source::hash::{FNV_OFFSET, fnv1a64_extend as fnv};

/// The length of magic, format version and kind.
const PREFIX_LEN: usize = 13;

impl R<'_> {
    fn fill(&mut self, out: &mut [u8]) -> Res<()> {
        if out.len() as u64 > self.left {
            return corrupt("truncated");
        }
        if self.src.read_exact(out).is_err() {
            return corrupt("truncated");
        }
        self.left -= out.len() as u64;
        if self.track {
            self.sum = fnv(self.sum, out);
        }
        Ok(())
    }
    fn array<const N: usize>(&mut self) -> Res<[u8; N]> {
        let mut a = [0u8; N];
        self.fill(&mut a)?;
        Ok(a)
    }
    fn u8(&mut self) -> Res<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn u32(&mut self) -> Res<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Res<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn bool(&mut self) -> Res<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => corrupt("bad flag byte"),
        }
    }
    /// A length or count whose elements take at least `min` bytes each:
    /// rejected unless that many bytes remain, so no allocation is sized
    /// by an unchecked value.
    fn count(&mut self, min: u64) -> Res<usize> {
        let n = self.u32()?;
        if u64::from(n).saturating_mul(min.max(1)) > self.left {
            return corrupt("length exceeds the file");
        }
        Ok(n as usize)
    }
    fn bytes(&mut self) -> Res<Vec<u8>> {
        let n = self.count(1)?;
        let mut v = vec![0u8; n];
        self.fill(&mut v)?;
        Ok(v)
    }
    fn str(&mut self) -> Res<String> {
        match String::from_utf8(self.bytes()?) {
            Ok(s) => Ok(s),
            Err(_) => corrupt("invalid UTF-8 in a string"),
        }
    }
    fn tag(&mut self) -> Res<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => corrupt("bad optional tag"),
        }
    }
    fn opt_u32(&mut self) -> Res<Option<u32>> {
        Ok(if self.tag()? { Some(self.u32()?) } else { None })
    }
    fn opt_u64(&mut self) -> Res<Option<u64>> {
        Ok(if self.tag()? { Some(self.u64()?) } else { None })
    }
    fn opt_str(&mut self) -> Res<Option<String>> {
        Ok(if self.tag()? { Some(self.str()?) } else { None })
    }
    fn pairs(&mut self) -> Res<Vec<(String, String)>> {
        let n = self.count(8)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push((self.str()?, self.str()?));
        }
        Ok(v)
    }
    /// Reads the rest of the body and the trailer: true if the checksum
    /// matches.
    fn checksum_ok(&mut self) -> Res<bool> {
        let mut buf = vec![0u8; 64 * 1024];
        while self.left > 0 {
            let n = usize::try_from(self.left.min(buf.len() as u64)).unwrap_or(buf.len());
            self.fill(&mut buf[..n])?;
        }
        let mut trailer = [0u8; 8];
        if self.src.read_exact(&mut trailer).is_err() {
            return corrupt("truncated");
        }
        Ok(u64::from_le_bytes(trailer) == self.sum)
    }
}

/// Checks the magic and the format version of the `len`-byte module file
/// `src`.
fn prefix(src: &mut dyn io::Read, len: u64) -> Res<R<'_>> {
    let mut p = [0u8; PREFIX_LEN];
    let have = usize::try_from(len.min(PREFIX_LEN as u64)).unwrap_or(PREFIX_LEN);
    if src.read_exact(&mut p[..have]).is_err() {
        return corrupt("truncated");
    }
    if !p[..have].starts_with(&MAGIC) {
        return Err(BmiError::NotKiln { clang: p[..have].starts_with(&CLANG_MAGIC) });
    }
    if have < PREFIX_LEN || len < (PREFIX_LEN + 8) as u64 {
        return corrupt("truncated");
    }
    let format = u32::from_le_bytes([p[8], p[9], p[10], p[11]]);
    if format != FORMAT_VERSION {
        return Err(BmiError::Format(format));
    }
    if p[12] != KIND_CAPSULE && p[12] != KIND_SECTIONS {
        return corrupt("unknown module file kind");
    }
    Ok(R {
        kind: p[12],
        src,
        left: len - (PREFIX_LEN + 8) as u64,
        sum: fnv(FNV_OFFSET, &p),
        track: true,
    })
}

fn header(r: &mut R<'_>) -> Res<Header> {
    let version = r.str()?;
    let sha = r.str()?;
    let module = r.str()?;
    let Some(kind) = UnitKind::from_code(r.u8()?) else {
        return corrupt("unknown unit kind");
    };
    if module.is_empty() {
        return corrupt("empty module name");
    }
    Ok(Header { version, sha, module, kind })
}

/// The header of a module file, without reading or checking the rest
/// (the checksum included): for the module name of
/// `-fmodule-file=<path>`.
///
/// # Errors
/// A [`BmiError`] for a file that is not a kiln module file of this
/// format, or whose header is malformed.
pub fn peek(bytes: &[u8]) -> Result<Header, BmiError> {
    let mut src = bytes;
    let mut r = prefix(&mut src, bytes.len() as u64)?;
    header(&mut r)
}

/// Reads a module file held in memory. Total: every malformed input is
/// an error.
///
/// # Errors
/// As [`read_from`].
pub fn read(bytes: &[u8]) -> Result<Capsule, BmiError> {
    let mut src = bytes;
    read_from(&mut src, bytes.len() as u64)
}

/// Reads the module file at `path` as it streams in ([`read_from`]).
///
/// # Errors
/// As [`read_from`]; a file that cannot be opened is
/// [`BmiError::Corrupt`] ("unreadable").
pub fn read_file(path: &Path) -> Result<Capsule, BmiError> {
    let Ok(f) = std::fs::File::open(path) else { return corrupt("unreadable") };
    let Ok(len) = f.metadata().map(|m| m.len()) else { return corrupt("unreadable") };
    let mut src = io::BufReader::with_capacity(64 * 1024, f);
    read_from(&mut src, len)
}

/// Reads the `len`-byte module file `src`. Total: every malformed input
/// is an error. A body that does not decode is reported as a checksum
/// mismatch when the checksum does not match either (damage), and by
/// what failed to decode otherwise.
///
/// # Errors
/// [`BmiError::NotKiln`] without the magic, [`BmiError::Format`] for
/// another layout version, [`BmiError::Corrupt`] for a bad checksum, a
/// truncated or malformed body, an out-of-range file index or trailing
/// bytes.
pub fn read_from(src: &mut dyn io::Read, len: u64) -> Result<Capsule, BmiError> {
    let mut r = prefix(src, len)?;
    let body = body(&mut r);
    let sum_ok = match r.checksum_ok() {
        Ok(ok) => ok,
        // A body error found before the file ran short explains it best.
        Err(e) => return Err(body.err().unwrap_or(e)),
    };
    if !sum_ok {
        return corrupt("checksum mismatch");
    }
    body
}

/// Decodes the body after the prefix; on success every body byte has
/// been read.
fn body(r: &mut R<'_>) -> Res<Capsule> {
    let h = header(r)?;
    let predefines = r.str()?;
    let fingerprint = r.pairs()?;
    let imports = r.pairs()?;
    let nfiles = r.count(8)?;
    let mut files = Vec::with_capacity(nfiles);
    for _ in 0..nfiles {
        files.push(CapsuleFile { path: r.str()?, text: r.str()? });
    }
    if files.is_empty() {
        return corrupt("no main file");
    }
    let n = u32::try_from(files.len()).unwrap_or(u32::MAX);
    let includer_ok = |i: u32| i < n || i == INCLUDER_BUILTIN || i == INCLUDER_NONE;
    let include_count = r.count(12)?;
    let mut include_log = Vec::with_capacity(include_count);
    for _ in 0..include_count {
        let includer = r.u32()?;
        let name = r.str()?;
        let angled = r.bool()?;
        let Some(kind) = kind_from_code(r.u8()?) else {
            return corrupt("unknown include kind");
        };
        let lookup_from = r.opt_u32()?;
        let hit = if r.tag()? {
            let file = r.u32()?;
            if file >= n {
                return corrupt("file index out of range");
            }
            Some(IncludeHit {
                file,
                lookup_index: r.opt_u32()?,
                system: r.bool()?,
                extern_c: r.bool()?,
                path: r.opt_str()?,
            })
        } else {
            None
        };
        if !includer_ok(includer) {
            return corrupt("includer index out of range");
        }
        include_log.push(IncludeEntry {
            key: IncludeKey { includer, name, angled, kind, lookup_from },
            hit,
        });
    }
    let embed_count = r.count(14)?;
    let mut embeds = Vec::with_capacity(embed_count);
    for _ in 0..embed_count {
        let includer = r.u32()?;
        let name = r.str()?;
        let angled = r.bool()?;
        let limit = r.opt_u64()?;
        let offset = r.opt_u64()?;
        let query_only = r.bool()?;
        let bytes = if r.tag()? { Some(r.bytes()?) } else { None };
        if !includer_ok(includer) {
            return corrupt("includer index out of range");
        }
        embeds.push(EmbedEntry {
            key: EmbedKey { includer, name, angled, limit, offset, query_only },
            bytes,
        });
    }
    let sections = if r.kind == KIND_SECTIONS { sections(r)? } else { Vec::new() };
    if r.left != 0 {
        return corrupt("trailing bytes");
    }
    Ok(Capsule {
        version: h.version,
        sha: h.sha,
        module: h.module,
        kind: h.kind,
        predefines,
        fingerprint,
        imports,
        files,
        includes: include_log,
        embeds,
        sections,
    })
}

/// The fingerprint fields that differ between a module file's `recorded`
/// fingerprint and the `current` compilation's, in recorded order, then
/// fields only the current one has: (recorded spelling, current
/// spelling), `"(none)"` for a missing field.
#[must_use]
pub fn mismatches(
    recorded: &[(String, String)],
    current: &[(String, String)],
) -> Vec<(String, String)> {
    let get = |v: &[(String, String)], k: &str| {
        v.iter().find(|(n, _)| n == k).map_or_else(|| "(none)".to_owned(), |(_, s)| s.clone())
    };
    let mut out = Vec::new();
    for (k, v) in recorded {
        let cur = get(current, k);
        if &cur != v {
            out.push((v.clone(), cur));
        }
    }
    for (k, v) in current {
        if !recorded.iter().any(|(n, _)| n == k) {
            out.push(("(none)".to_owned(), v.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Capsule {
        Capsule {
            version: KILN_VERSION.to_owned(),
            sha: "abc".to_owned(),
            module: "m:p".to_owned(),
            kind: UnitKind::InterfacePartition,
            predefines: "#define __kiln__ 1\n".to_owned(),
            fingerprint: vec![("std".into(), "-std=c++20".into())],
            imports: vec![("n".into(), "n.pcm".into())],
            files: vec![
                CapsuleFile { path: "m.cppm".into(), text: "export module m:p;\n".into() },
                CapsuleFile { path: "h.h".into(), text: "int x;\n".into() },
            ],
            includes: vec![
                IncludeEntry {
                    key: IncludeKey {
                        includer: 0,
                        name: "h.h".into(),
                        angled: false,
                        kind: IncludeKind::Include,
                        lookup_from: None,
                    },
                    hit: Some(IncludeHit {
                        file: 1,
                        lookup_index: Some(2),
                        system: true,
                        extern_c: false,
                        path: Some("./h.h".into()),
                    }),
                },
                IncludeEntry {
                    key: IncludeKey {
                        includer: INCLUDER_BUILTIN,
                        name: "nope".into(),
                        angled: true,
                        kind: IncludeKind::HasIncludeNext,
                        lookup_from: Some(1),
                    },
                    hit: None,
                },
            ],
            embeds: vec![EmbedEntry {
                key: EmbedKey {
                    includer: 0,
                    name: "d.bin".into(),
                    angled: false,
                    limit: Some(3),
                    offset: None,
                    query_only: false,
                },
                bytes: Some(vec![1, 2, 3]),
            }],
            sections: vec![],
        }
    }

    /// A kind-2 file: [`sample`] with a carried-constants section.
    fn sample2() -> Capsule {
        let mut c = sample();
        c.sections.push(Section { tag: SECTION_CONSTS, bytes: vec![7, 0, 0, 0, 9] });
        c
    }

    /// Re-frames `body` (a file without its checksum) with a valid one.
    fn resum(mut body: Vec<u8>) -> Vec<u8> {
        let s = checksum(&body);
        body.extend_from_slice(&s.to_le_bytes());
        body
    }

    #[test]
    fn round_trip() {
        let c = sample();
        let b = write(&c);
        assert_eq!(read(&b), Ok(c.clone()));
        assert_eq!(write(&c), b, "deterministic");
        let h = peek(&b).unwrap();
        assert_eq!((h.module.as_str(), h.kind), ("m:p", UnitKind::InterfacePartition));
    }

    #[test]
    fn foreign_and_version() {
        assert_eq!(read(b"CPCH\x01\x02"), Err(BmiError::NotKiln { clang: true }));
        assert_eq!(read(b"int main;"), Err(BmiError::NotKiln { clang: false }));
        assert_eq!(read(b""), Err(BmiError::NotKiln { clang: false }));
        let mut b = write(&sample());
        b[8] = 9;
        assert_eq!(read(&b), Err(BmiError::Format(9)));
    }

    /// Every truncation and every single-byte flip is an error, never a
    /// panic, and a flip is always caught (by the checksum at the latest).
    #[test]
    fn truncate_and_flip() {
        let b = write(&sample());
        for n in 0..b.len() {
            assert!(read(&b[..n]).is_err(), "truncated to {n}");
            let _ = peek(&b[..n]);
        }
        for i in 0..b.len() {
            for bit in [0x01u8, 0x80, 0xff] {
                let mut m = b.clone();
                m[i] ^= bit;
                assert!(read(&m).is_err(), "flip {bit:#x} at {i}");
                let _ = peek(&m);
            }
        }
    }

    /// With the checksum recomputed, a corrupted body still never panics
    /// (the structural checks are total on their own).
    #[test]
    fn corrupt_body_with_valid_checksum() {
        let b = write(&sample());
        let body = b.len() - 8;
        let mut seed: u32 = 0x9e37_79b9;
        for i in 13..body {
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let mut m = b[..body].to_vec();
                m[i] = seed.to_le_bytes()[0];
                let s = checksum(&m);
                m.extend_from_slice(&s.to_le_bytes());
                let _ = read(&m);
            }
            let mut m = b[..body].to_vec();
            m[i..].iter_mut().take(4).for_each(|x| *x = 0xff);
            let s = checksum(&m);
            m.extend_from_slice(&s.to_le_bytes());
            let _ = read(&m);
        }
    }

    /// Kind 2 round-trips, is deterministic, and is kind 1 plus the
    /// sections: the kind byte is the only other difference.
    #[test]
    fn kind2_round_trip() {
        let c = sample2();
        let b = write(&c);
        assert_eq!(b[12], KIND_SECTIONS);
        assert_eq!(read(&b), Ok(c.clone()));
        assert_eq!(write(&c), b, "deterministic");
        assert_eq!(peek(&b).unwrap().module, "m:p");
        let one = write(&sample());
        assert_eq!(one[12], KIND_CAPSULE);
        assert_eq!(b[13..one.len() - 8], one[13..one.len() - 8]);
    }

    /// Kind 2: every truncation and flip is an error, never a panic.
    #[test]
    fn kind2_truncate_and_flip() {
        let b = write(&sample2());
        for n in 0..b.len() {
            assert!(read(&b[..n]).is_err(), "truncated to {n}");
        }
        for i in 0..b.len() {
            for bit in [0x01u8, 0x80, 0xff] {
                let mut m = b.clone();
                m[i] ^= bit;
                assert!(read(&m).is_err(), "flip {bit:#x} at {i}");
            }
        }
    }

    /// Behind a valid checksum: a kind-2 file without sections, an
    /// unknown (reserved) section tag, sections out of order, and a kind-1
    /// file with trailing section bytes are all errors.
    #[test]
    fn kind2_section_framing() {
        let one = write(&sample());
        let body1 = one[..one.len() - 8].to_vec();
        let with = |kind: u8, tail: &[u8]| {
            let mut m = body1.clone();
            m[12] = kind;
            m.extend_from_slice(tail);
            read(&resum(m))
        };
        let sec = |tag: u32, bytes: &[u8]| {
            let mut v = tag.to_le_bytes().to_vec();
            v.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            v.extend_from_slice(bytes);
            v
        };
        let count = |n: u32| n.to_le_bytes().to_vec();
        assert!(with(KIND_SECTIONS, &count(0)).is_err(), "no sections");
        assert!(with(KIND_SECTIONS, &[count(1), sec(0, &[1])].concat()).is_err(), "tag 0");
        let reserved = [count(1), sec(SECTION_MAX + 1, &[1])].concat();
        assert!(with(KIND_SECTIONS, &reserved).is_err(), "reserved tag");
        let twice = [count(2), sec(1, &[1]), sec(1, &[2])].concat();
        assert!(with(KIND_SECTIONS, &twice).is_err(), "out of order");
        assert!(with(KIND_SECTIONS, &[count(1), sec(1, &[])].concat()).is_ok(), "empty section");
        assert!(with(KIND_CAPSULE, &[count(1), sec(1, &[1])].concat()).is_err(), "kind 1");
        assert!(with(3, &[]).is_err(), "unknown kind");
    }

    #[test]
    fn fingerprint_mismatches() {
        let rec =
            vec![("std".to_owned(), "-std=c++20".to_owned()), ("rtti".into(), "-frtti".into())];
        let cur = vec![("std".to_owned(), "-std=c++23".to_owned()), ("x".into(), "-fx".into())];
        assert_eq!(
            mismatches(&rec, &cur),
            [
                ("-std=c++20".to_owned(), "-std=c++23".to_owned()),
                ("-frtti".into(), "(none)".into()),
                ("(none)".into(), "-fx".into()),
            ]
        );
        assert!(mismatches(&rec, &rec).is_empty());
    }

    /// The section-block checksum (WP-1392): pinned, length-sensitive,
    /// and any one flipped bit of a block (in a whole 32-byte group or in
    /// the tail) changes it, so `read_sections` reports it as damage.
    /// [`BlockChecksum`] (WP-1393) over any split of a block is
    /// [`block_checksum`] of the whole, and both keep the values written
    /// PCHs carry (pinned from the one-pass definition).
    #[test]
    fn block_checksum_by_pieces_is_the_whole() {
        let data: Vec<u8> = (0..101u32).map(|i| (i * 37 % 251) as u8).collect();
        assert_eq!(block_checksum(b""), 0xf959_3e91_de54_6f77);
        assert_eq!(block_checksum(&data), 0x96bb_e995_0df2_a5a5);
        for cut in [1, 7, 31, 32, 33, 64, 100] {
            for step in [1, 3, 32, 50, 101] {
                let mut c = BlockChecksum::new();
                c.update(&data[..cut]);
                for piece in data[cut..].chunks(step) {
                    c.update(piece);
                }
                assert_eq!(c.finish(), block_checksum(&data), "cut {cut} step {step}");
            }
        }
    }

    #[test]
    fn block_checksum_catches_every_flip() {
        assert_eq!(block_checksum(b""), block_checksum(b""));
        assert_ne!(block_checksum(b""), block_checksum(b"\0"));
        assert_ne!(block_checksum(&[0; 32]), block_checksum(&[0; 64]));
        let data: Vec<u8> = (0..101u32).map(|i| (i * 37 % 251) as u8).collect();
        let sum = block_checksum(&data);
        for i in 0..data.len() {
            for bit in 0..8 {
                let mut d = data.clone();
                d[i] ^= 1 << bit;
                assert_ne!(block_checksum(&d), sum, "byte {i} bit {bit}");
            }
        }
        let block = write_sections(&[
            Section { tag: SECTION_PP, bytes: data.clone() },
            Section { tag: SECTION_SNAPSHOT, bytes: vec![1, 2, 3] },
        ]);
        let back = read_sections(&block).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].bytes, data);
        for i in 0..block.len() {
            let mut b = block.clone();
            b[i] ^= 0x10;
            assert!(read_sections(&b).is_err(), "byte {i}");
        }
        assert!(read_sections(&block[..block.len() - 1]).is_err());
        assert!(read_sections(&block[..3]).is_err());
    }

    /// `-o /dev/null` writes through the device; renaming a temporary file
    /// over it (as root) would replace /dev/null with a regular file.
    #[cfg(unix)]
    #[test]
    fn write_file_keeps_a_device_output_a_device() {
        use std::os::unix::fs::FileTypeExt as _;
        let null = Path::new("/dev/null");
        write_file(null, b"module bytes").unwrap();
        assert!(std::fs::metadata(null).unwrap().file_type().is_char_device());
    }
}
