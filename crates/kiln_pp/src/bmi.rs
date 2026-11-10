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
/// The highest section tag this reader knows. Tags above it are reserved
/// for later parts of the serialised AST (§11.11, "Sections").
pub const SECTION_MAX: u32 = SECTION_IMAGE;
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
    /// The unit's main file, loaded as the replay loads it (§11.2): the
    /// image's locations are offsets into it.
    pub main_file: kiln_source::FileId,
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

/// The main file path and the gated names an image section lists up
/// front (spec/modules.md §11.11 "Part 2a", "Encoding"): the names of
/// its declarations at global namespace scope, which the importer's
/// preprocessor checks against the names it has seen there before it uses
/// the image; `None` when the header does not decode (the reader then
/// reports the file as corrupt).
pub fn image_header(bytes: &[u8]) -> Option<(&str, Vec<&str>)> {
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
    Some((main, gated))
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
        let mut sorted: Vec<&Section> = c.sections.iter().collect();
        sorted.sort_by_key(|s| s.tag);
        w.len(sorted.len());
        for s in sorted {
            w.u32(s.tag);
            w.bytes(&s.bytes);
        }
    }
    let sum = checksum(&w.0);
    w.u64(sum);
    w.0
}

/// Writes `bytes` to `path` atomically: to a temporary file beside it,
/// then renamed over it, so an interrupted compile never leaves a
/// half-written module file.
///
/// # Errors
/// The I/O error of creating, writing or renaming the file.
pub fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
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
}

type Res<T> = Result<T, BmiError>;

fn corrupt<T>(why: &str) -> Res<T> {
    Err(BmiError::Corrupt(why.to_owned()))
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

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
        self.sum = fnv(self.sum, out);
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
    Ok(R { kind: p[12], src, left: len - (PREFIX_LEN + 8) as u64, sum: fnv(FNV_OFFSET, &p) })
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
    let mut sections = Vec::new();
    if r.kind == KIND_SECTIONS {
        let n = r.count(8)?;
        if n == 0 {
            return corrupt("a kind-2 file without sections");
        }
        sections.reserve(n);
        for _ in 0..n {
            let tag = r.u32()?;
            if tag == 0 || tag > SECTION_MAX {
                return corrupt("unknown section");
            }
            if sections.last().is_some_and(|s: &Section| s.tag >= tag) {
                return corrupt("sections out of order");
            }
            sections.push(Section { tag, bytes: r.bytes()? });
        }
    }
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
}
