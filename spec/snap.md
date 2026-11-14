# spec: kiln_snap, kiln_snap_derive

The snapshot encoding: how a crate's in-memory state is written to bytes
and read back as an equal state, for the PCH snapshot (WP-1283: the
compiler state after a prefix of a translation unit, restored by a later
compilation instead of re-parsing the prefix). Status: prototype (WP-1377),
piloted on `kiln_source`, extended to `kiln_diag`, `kiln_lex` and `kiln_pp`
(WP-1379); nothing in the compiler calls it yet.

`kiln_snap` is the runtime (trait, wire format, standard impls, errors,
schema hash). `kiln_snap_derive` is the `#[derive(Snap)]` proc macro, on
the compiler's own `proc_macro` API with no dependency. `kiln_snap`
re-exports the derive, so a crate depends on `kiln_snap` only.

## Model

A snapshot is the bytes of one value: a tree of plain data written field
by field, in declaration order, with no field names, padding or alignment.
Every cross-reference in kiln state is a `u32` arena index (`FileId`,
`Symbol`, AST ids), so the encoding of an index is the index: no pointer
fix-up and no identity table. The writer and the reader are the same build
(the schema hash checks this), so the format has no versioning and no
optional fields.

Restoring is not "read some bytes and continue": a type whose state is
more than its data (locks, caches keyed by machine state, closures,
derived lookup tables) is a **root**, and its encoding is written by hand
(below). Everything else derives.

## The trait

```rust
pub trait Snap: Sized + 'static {
    const SIZE: Option<usize> = None;
    const OPTION_SIZE: Option<usize> = /* SIZE + 1 */;
    const SCHEMA_HASH: u64;                                    // required
    const SHAPE_HASH: u64 = Self::SCHEMA_HASH;                 // own items only
    fn put(&self, w: &mut Writer);
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError>;
    fn schema(s: &mut Schema);
    fn put_option(v: Option<&Self>, w: &mut Writer);         // default: tag + value
    fn get_option(r: &mut Reader<'_>) -> Result<Option<Self>, SnapError>;
}
pub fn to_bytes<T: Snap>(v: &T) -> Vec<u8>;
pub fn to_bytes_canonical<T: Snap>(v: &T) -> Vec<u8>;
pub fn from_bytes<T: Snap>(bytes: &[u8]) -> Result<T, SnapError>; // all bytes consumed
pub fn put_seq<'x, T: Snap>(w: &mut Writer, len: usize, items: impl IntoIterator<Item = &'x T>);
pub fn schema_hash<T: Snap>() -> u64;
```

The contract an impl keeps (the derive keeps it by construction):

- `get` reads exactly what `put` wrote and returns a value equal to the
  one written, for every value, in both modes.
- `get` never panics, never allocates more than the input can justify
  (a length is checked against the remaining bytes before any
  allocation), and reports bad input as a [`SnapError`](#errors).
- `SIZE = Some(n)`: every `put` writes exactly `n` bytes, and the type
  holds no `Rc`/`Arc` (the bulk path decodes items out of order of the
  sharing table). `SIZE = None`: every `put` writes at least one byte
  (this bounds a sequence's length by the remaining bytes).
- `schema` registers the type once and recurses into every type it
  encodes, and `SCHEMA_HASH` folds the same facts at compile time
  ([Schema hash](#schema-hash)).
- `put` takes `&self`: encoding requires quiescence (no other thread
  appending; a `RefCell` mutably borrowed during `put` panics, documented
  at its impl; held locks are waited for).

## Wire format

- Integers: fixed width, little-endian. `usize`/`isize` are 8 bytes on
  every target. Floats: their bits. `bool`: one byte, 0 or 1. `char`:
  4 bytes, a valid scalar value.
- Lengths (sequences, strings, maps, sharing references): unsigned LEB128.
- `String`/`str`: length, then UTF-8 (checked on read). `PathBuf`: the
  raw OS bytes (Unix).
- `Option<T>`: one tag byte (0 `None`, 1 `Some`), then the value; when `T`
  has a fixed size, `None` is followed by `SIZE` zero bytes, so
  `Option<T>` is fixed-size too. A type with a niche overrides
  `put_option`/`get_option` and `OPTION_SIZE`: `NonZero*` writes 0 for
  `None`, so `Option<NonZeroU32>` is 4 bytes, and a derived single-field
  newtype forwards to its field, so `Option<Id>` for `struct Id(NonZeroU32)`
  is 4 bytes as well.
- Structs and tuples: the fields in order. `SIZE` is the sum when every
  field is fixed, else `None`.
- Enums: the tag is the variant's **index in declaration order** (not its
  discriminant), `u8` up to 256 variants, then `u16`, `u32`; then the
  variant's fields. When every variant's fields are fixed-size, every
  variant is zero-padded to the largest, so the enum is fixed-size
  (`SIZE = tag + max`). Padding (here and after `None`) is written as
  zeros and checked on read (`Reader::skip_zeros`): a nonzero pad byte
  is `BadValue("nonzero padding")` at that byte.
- `Vec<T>`, `Box<[T]>`, `VecDeque<T>`, `[T; N]` (no length): length, then
  the items.
- `Box<T>`: `T`, with `SIZE = None` (so recursive types terminate).
- `Cell`, `RefCell`, `Mutex`, `RwLock` (poison ignored), atomics
  (`Relaxed`): the inner value. `OnceCell`/`OnceLock`: as `Option`.
- `Rc`/`Arc` of `T`, `[T]`, `str`: [sharing](#sharing).
- Hash maps and sets (std and `hashbrown`, any `BuildHasher: Default`):
  [maps](#hash-maps). `BTreeMap`/`BTreeSet`: length, then entries in
  order; a duplicate key on read is an error.
- Not provided: references and lifetimes (a snapshot owns its data),
  `Weak`, raw pointers, `hashbrown::HashTable` (it stores no hasher: its
  owner writes the entries and rebuilds it).

### The bulk path

`put_seq` and `Reader::seq_items` are how every sequence is written and
read. For `T::SIZE = Some(s)` the reader checks `n * s` bytes once and
decodes each `s`-byte chunk from a sub-reader with no per-field bounds
check that can fail (the derive's `get` still runs, so invalid values
such as a bad `bool` are still errors). For `SIZE = None`, `n` is bounded
by the remaining bytes and each item is read in turn. A crate-local
sequence type (`kiln_source`'s `AppendVec`) implements `Snap` with
`put_seq`/`seq_len`/`seq_items` and gets the same path.

### Streaming reads

`Reader::new(&[u8])` reads a slice. `Reader::streaming(src, len, base)`
reads the next `len` bytes of any `std::io::Read` with the same results
and errors, so a large snapshot is decoded without a copy of it in
memory (WP-1393: the PCH consumer streams the roots from the PCH file).
The reader keeps a window of the source: a read needing more than the
window holds drops the bytes already decoded and reads at least
`WINDOW` (1 MiB) more, or what the read needs. `bytes` and `str`
borrow the reader (valid until its next read), so no read may hold on to
the input; the fixed-size bulk path takes at most a window's worth of
items per bounds check. `remaining` counts the bytes still in the
source, so the length checks of `seq_len` are the same in both modes. A
source that fails or ends before `len` bytes is `ErrorKind::Io` at the
offset the bytes were needed; it never panics. The window is kept inline
in the reader, so a slice reader's reads cost one select more than
before, not an indirection.

### Sharing

An `Rc`/`Arc` is written as a LEB128 reference: `0` followed by the
pointee for the first occurrence of an allocation, `i + 1` for a later
occurrence of the `i`-th shared allocation written (numbered in pre-order
across the whole snapshot, keyed by address and pointer type). The reader
hands out clones of one new allocation, so `Rc::ptr_eq` holds after the
restore exactly where it held before. A cycle (an `Rc` reachable from its
own pointee) encodes but is refused on read (`SharedCycle`); state with
cycles is a root and breaks them by index.

### Hash maps

Normal mode writes the capacity, the length and the entries in iteration
order; the reader allocates `min(capacity, remaining bytes)` (at least the
length) and inserts in that order. With the same hasher this usually
reproduces the iteration order, but it is **not guaranteed** (a slot freed
by a removal can take an entry earlier than its original probe did; the
group width is per target): measured on `FxHashMap<u32, u32>` with every
third key removed, 10 to 100,000 entries, the order was equal in every
case (`crates/kiln_source/tests/snap.rs`). Code whose output depends on
map order already sorts or uses an ordered map; snapshots do not change
that rule.

**Canonical mode** (`Writer::canonical`, `to_bytes_canonical`) writes
maps and sets sorted by each entry's encoded bytes, with the length in
place of the capacity: two equal states give equal bytes whatever their
history. It is for tests and for content hashing, not for the snapshot
file (it costs one scratch encode per entry). A duplicate key on read is
an error in both modes.

## Errors

`SnapError` is one boxed word: an `ErrorKind` (`Truncated`, `BadTag`,
`BadValue`, `LengthTooLarge`, `SharedOutOfRange`, `SharedWrongType`,
`SharedCycle`, `TrailingBytes`, `Invalid(String)`, `Io(String)` of a
streaming reader; non-exhaustive), the
type being read, the absolute byte offset, and the field path. The derive
adds `Type.field` / `Enum::Variant.field` to the path of an error from a
field (`SnapError::within`), so a corrupt snapshot reports e.g.
`snapshot: bad tag 7 reading `kiln_source::FileKind` at byte 1234 (in
SourceManager.kinds > ...)`. A root adds its own path elements the same
way. Decoding never panics on any input (tests truncate at every byte and
flip bits at every byte).

### Id ranges

A decoder can only check an index against a table it has already read,
and the derive checks none: an id is a field like any other. The
`Reader` therefore carries one running maximum per *id space*
(`ID_SPACES` = 32): an id type's hand-written `get` calls
`Reader::note_id(space, index)`, and the root that owns the space reads
`Reader::id_max(space)` once its tables are decoded and refuses the
snapshot (`Invalid`) when an id read so far exceeds them. A later root
holding ids into an earlier one checks again after its own decode. The
bulk path's per-item sub-reader carries the maxima through the block
(with the streaming decode, from window block to window block). The id
types' descriptor is one `id` item, and their `SCHEMA_HASH` folds that
item and the type's name: the check is on decode, not in the encoding.
Cost: one compare per id decoded (WP-1396: under the timer's noise on
the libstdc++ prefix). The spaces in use are kiln_ast's (`ids::space`,
0-18; spec/ast.md "Snapshots").

## Schema hash

`T::schema` registers a descriptor per type reachable from `T`:
`"{type_name} = {kind} {field}:{field type} ... | {Variant}={disc} ..."`,
with `skip` and `with(path)` marked. `schema_hash::<T>()` is FNV-1a over
the sorted descriptors. Descriptors hold only names, so the hash is equal
across builds of the same sources and changes when any reachable field is
added, removed, renamed, reordered or retyped, a variant changes, or an
attribute changes the encoding. A hand-written impl whose encoding
changes without a field change bumps its `Desc::note`.

`Snap::SCHEMA_HASH` (WP-1401) is the same fact as a constant, so a load
walks nothing: a `SchemaHash` const-fn builder folds, in the order the
descriptor lists them, the kind and the type's own name, each field's
name with its type's `SCHEMA_HASH` (or `skip` / `with(path)` with the
spelled type), each variant's name and discriminant, and notes. A field
that closes a cycle of types, one whose type names the type itself
(`Box<Self>`, `Vec<Node>`) or one marked `#[snap(cycle)]` (`InitValue`'s
`InitList`, which reaches `InitValue` again), folds its type's
`SHAPE_HASH` instead (`SchemaHash::cycle`): the type's own items, with
a derived type's fields by their spelling and a container's elements by
their `SHAPE_HASH`, so it depends on no constant of the cycle. A cycle
left open is a compile error (E0391) at the constants involved.
`Schema::unfolded` names, for each root of a walk, the reached types
whose items its constant misses because a cycle is closed at the wrong
place; the driver's test asserts it is empty. A hand-written root defines its
`SNAP_SCHEMA_HASH` the same way and folds subordinate roots with
`root_field`; the configured `Sema` root folds its registered fields'
types through `reg_hash`, which infers each from a field projection.
Every `Desc` carries its type's constant (`Desc::hash`), and the driver's
`schema_constants_cover_every_type` test walks all snapshot roots and
checks that every reached type has a constant and that two different
descriptors never share one: a constant missing a field, an attribute
or a type parameter the descriptor names fails it. The driver stores
`pch_snapshot::SCHEMA_HASH`, folded over its five roots; a snapshot with
another is refused, never misread.

## The derive

`#[derive(Snap)]` on named, tuple and unit structs and on enums (unit,
tuple and struct variants, explicit discriminants, any `repr`), with type
and const generics and `where` clauses. It adds `T: Snap` to every type
parameter. It generates `put` as one exhaustive destructure (`let Self {
a, b, c } = self;`, no `..`) and `get` as a full struct literal, so adding
a field to a derived type without deciding how it is snapshotted is
impossible: the field is either encoded (it needs `Snap`) or marked.

Attributes:

- `#[snap(skip)]` on a field: not written; read back as
  `Default::default()`. Use for caches that refill on demand.
- `#[snap(with = "path::module")]` on a field: written by
  `module::put(&T, &mut Writer)`, read by `module::get(&mut Reader<'_>) ->
  Result<T, SnapError>` (the field's size becomes `None`).
- `#[snap(cycle)]` on an encoded field: the field's type reaches this
  type again; the schema constant folds the field type's `SHAPE_HASH`
  ([Schema hash](#schema-hash)). Encoding unchanged.
- `#[snap(bound = "T: Trait, ...")]` on the type: replaces the automatic
  `T: Snap` bounds (for a parameter used only in `PhantomData`, or behind
  an associated type).

Errors are `compile_error!` at the offending token: a lifetime parameter,
a union, an unknown or misplaced attribute (`skip`/`with` on the type or a
variant, `bound` on a field), `skip` together with `with`; a field type
without `Snap` is the ordinary trait error at the field. The cases are
doc tests in `kiln_snap` (`derive_errors`).

## Hand-written roots

A type is a root when restoring it is more than reading its fields back:
it holds locks or atomics that are hints, caches keyed by machine state
(inode numbers, addresses), closures, lookup tables derived from other
fields, or configuration owned by the consumer's command line. A root:

1. Starts its `put` with an exhaustive destructure of `self` and builds
   its value in `get` with a full struct literal, as the derive does, so a
   new field is a compile error in the root until it is classified.
2. Classifies **every** field in a comment table at the impl, as one of
   - **state**: written, as data;
   - **rebuilt**: not written; recomputed in `get` from the state (a hash
     map from strings to ids, a path index from `stat`), or on first use;
   - **transient**: not written; reset (lookup hints, thread-local
     caches);
   - **configuration**: not written; set by the consumer from its own
     options, which the snapshot's configuration fingerprint covers.
3. Validates in `get` every cross-table invariant the type's queries rely
   on without a check (ids in range, tables tiling their index space,
   links pointing backwards), so that a decoded value can be queried
   without a panic; a failure is `ErrorKind::Invalid` with a short reason.
4. Keeps its id allocation identical: restoring then adding content gives
   the ids and locations that adding the same content to the original
   would.

The roots so far:

- `kiln_source::SourceManager` (`src/manager/snap.rs`) and
  `kiln_source::symbol::Interner` (`src/symbol/snap.rs`), the pilots;
  spec/source.md "Snapshots" states their behaviour.
- `kiln_ast::Ast` (`src/ast/snap.rs`) with `TypeContext` and its
  `KeyedTable`, `ArgTable`, `ProfileTable` (`src/types/snap.rs`),
  `ConstStore` (`src/consts/snap.rs`), `StringPool` (`src/strings.rs`)
  and `ColdMap` (`src/slot.rs`); spec/ast.md §17 states their behaviour.
  `kiln_abi`'s target and layout types derive `Snap` (no root).
- `kiln_oven::Oven` (`src/snap.rs`, with `Memory` in `src/mem/snap.rs`),
  a **configured** root: its limits and options are the consumer's, so
  it is not `Snap`. The consumer builds the Oven as it would without a
  snapshot and applies one to it: `snap_ready` (the quiescence check),
  `snap_put` (refused unless ready; writes nothing then), `snap_restore`
  (into an unused Oven; validates everything before assigning anything)
  and `snap_schema`; spec/oven.md "Snapshots" states its behaviour.
- `kiln_sema::Sema` (`src/snap.rs`), a *configured* root (WP-1382;
  spec/sema.md §8b): it borrows the diagnostics engine and holds the
  consumer's options, so it is not `Snap` but has `snap_put(&self, &mut
  Writer)`, `snap_restore(&mut self, &mut Reader)` applied to a fresh
  object the consumer built, `snap_schema`, and a quiescence check
  (`snapshot_ready`). Its AST and Oven are written by their own roots.

What the Sema root (WP-1382) added to the recipe:

- More classes than the four above: *scratch* (a reused buffer),
  *reference* (a borrowed object, the consumer's) and *owned root*
  (another crate's root inside this one). A root with a hundred fields
  names them in one `macro_rules!` list per class, from which the
  destructure, the writes, the schema and the at-rest checks are
  generated.
- A transient field is checked, not assumed: the quiescence check names
  the first one not as a fresh object has it.
- Configuration inside a state field (a limit inside a state struct) is
  saved from the consumer's object before the restore and put back.
- A field type from a crate the WP may not touch (`Range<u32>`) gets a
  `#[snap(with = ...)]` module, not an impl.
- A shared `Arc` (the interner) is restored as the same allocation only
  when every holder is in the same stream after its first writer; a root
  that needs the consumer's allocation checks `ptr_eq` and refuses.

### Configured roots

A root that borrows other roots or holds configuration that cannot be
read from bytes (trait objects, closures, a sink, options from the
consumer's command line) is **configured**: it is not `Snap`. The
consumer builds it from its own command line, exactly as without a
snapshot, and the snapshot is applied to it. It has four inherent
methods, named so in every crate:

```rust
pub fn snap_ready(&self) -> Result<(), String>;
pub fn snap_put(&self, w: &mut Writer) -> Result<(), String>;
pub fn snap_restore(&mut self, r: &mut Reader<'_>) -> Result<(), SnapError>;
pub fn snap_schema(s: &mut Schema);   // associated, no `self`
```

- `snap_ready` is the **quiescence check**: it inspects every field
  classified transient-at-rest and returns `Err("Type.field: why")` for
  the first one that is not at its resting value (an open stack, a
  queued item, a flag set mid-operation). It never panics.
- `snap_put` calls `snap_ready` and writes nothing when it fails;
  otherwise it writes the state fields in a fixed order (the writer's
  mode decides map order, so `Writer::canonical()` gives the bytes tests
  compare). It starts with an exhaustive destructure of `self`.
- `snap_restore` applies a snapshot to a **fresh** object (built, and
  configured, but not used: the method defines what "used" means and
  refuses it with `ErrorKind::Invalid`). It decodes and validates every
  field first and assigns them only when all succeeded, so on an error
  the object is unchanged. Fields classified rebuilt or configuration
  keep what the consumer's constructor gave them; transient ones are
  already at rest in a fresh object. It ends with an exhaustive
  destructure (or assigns every state field by name).
- `snap_schema` registers a `"configured root"` descriptor for the type
  with its lifetimes `'static` (`Type<'static>`), naming every written
  field (`Desc::field`), then recurses into each field type, as a
  hand-written root's `schema` does. Its hash is
  `{ let mut s = Schema::new(); Type::snap_schema(&mut s); s.hash() }`.
- The **classification table** (rule 2 above, with "transient, at rest"
  for what `snap_ready` checks) is at the impl, in a `snap` submodule.
- **Restore order**: roots that others borrow are restored first; a
  configured root is restored before anything borrows it mutably or
  reads it. The interner before anything interns, the source manager
  before any file is loaded, the diagnostics engine before the
  preprocessor is built.

The configured roots are `kiln_diag::DiagEngine` (`src/engine/snap.rs`,
spec/diag.md §7.8) and `kiln_pp::Preprocessor` (`src/snap.rs`, spec/pp.md
"Snapshots").

### Whole-compiler restore order

The PCH snapshot (WP-1383, the default PCH path since WP-1404, `kiln_driver/src/pch_snapshot.rs`,
docs/status/pch-snapshot-prototype.md) writes the whole front end at the
end of a prefix as one stream, so `Arc<Interner>` sharing between the
roots is kept. The producer parses an empty main file whose `-include`s
are its own and the header (`kiln_parse::parse_prefix`), checks
`ready`, closes the main file (`Preprocessor::end_main_file`) and writes,
in this order:

1. `SourceManager` (plain `Snap`);
2. `Arc<Interner>` (plain `Snap`);
3. `DiagEngine::snap_put`;
4. `Preprocessor::snap_put`;
5. `Sema::snap_put` (the `Ast`, then the Oven, then Sema's own fields).

The consumer restores in the same order, and builds each configured
root from its own command line just before applying the snapshot to it:

1. the source manager and the interner, before anything loads a file or
   interns a name;
2. the diagnostics engine (`make_engine` over the restored source
   manager), then `snap_restore`;
3. the main file loaded into the restored source manager (it gets the
   next file id after the producer's);
4. the preprocessor configured as without a snapshot (resolver, `#if`
   evaluator, feature queries, module options) but with no main file
   entered, then `snap_restore`, then `enter_next_main_file(main)`; the
   consumer's own predefines are not read again (they are state), and
   its `-include`s after the PCH are entered as a new `<built-in>`
   buffer (`Preprocessor::enter_builtin_includes`);
5. a fresh `Sema` (`kiln_parse::new_sema`, with the consumer's
   `ParseOptions` and constexpr limits), then `snap_restore` (the
   `Ast`, the Oven, then Sema's own fields);
6. the reader must be at its end; then
   `kiln_parse::continue_preprocessed` parses the main file and the
   rest of the job (end-of-translation-unit work, codegen) runs as
   without a snapshot.

Any decoding or validation error before step 6 drops everything the
restore built, and the job starts again by replaying the header; the
error is reported only by `-v`. The section is used only when its head
matches: its magic and format version, the schema hash of all five
roots, and the configuration fingerprint (spec/driver.md "Snapshots",
the identity table). The consumer streams the
roots from the PCH file ("Streaming reads"): the section block is
located and its checksum checked when the PCH is read, and only the
roots' offset is kept, so the compile holds no copy of it. The restore
checks the file's size and modification time again when it opens it,
and reads the whole block again through the same checksum
(`bmi::BlockChecksum`, incremental `bmi::block_checksum`): the bytes
before the roots when it opens, the roots as they are decoded. Once the
last root is decoded, and before anything restored is used (the main
file is not yet parsed), the checksum must be the block's; otherwise, as
for any other change, the compile replays.

### The producer's single pass

The producer also checks the header as clang does: a pass with the
header as the main file (its diagnostics, its errors, its dependency
list). That pass and the snapshot pass read the same files in the same
order; they differ only in the header's place. So when the command line
asks for nothing beyond diagnostics and dependencies
(`pch_snapshot::single_pass_allowed`: no `-v`, `-verify`, time trace,
declaration image, report, dump, module output or `-MG`, and a C++
header: a C header takes two passes, since C has a main-file-only
diagnostic, an internal declaration used in an extern inline function,
that the list below does not cover), the snapshot
pass runs first, with its diagnostics counted instead of shown, and goes
on as a consumer with an empty main file would (`enter_next_main_file`,
then `continue_preprocessed`: the end-of-translation-unit work). If it
ends with no error and nothing written, and nothing a main file alone
gets could differ (`pch_snapshot::stands_in`), it stands in for the
header pass: its dependency list, with the header moved to the front,
is the header pass's. What a main file alone gets, and how each is ruled
out:

- `#include_next` in the primary file and `#pragma system_header` in
  the main file are warned of, and module or import lines are read only
  in a main file: no such line in the header;
- unused internal declarations are warned of in the main file only
  (`-Wunused-variable`, `-Wunused-const-variable`,
  `-Wunneeded-internal-declaration`, `-Wunused-template`): those
  warnings off, or a header of directives and comments only; and no
  `#pragma ... diagnostic` in the header;
- `__INCLUDE_LEVEL__` and `__BASE_FILE__` expand differently: in no
  file read;
- the dependency list names the main file first and never as a system
  header: the header listed under its own name, not as a system header.

Otherwise the header pass runs as before (its output is the only one
shown), and the snapshot already made is used. A failed snapshot pass
(an error in the prefix, a prefix not at rest) is the same as before: no
section, and the header pass reports. `KILN_PCH_SNAPSHOT_PASSES=2`
forces the two passes, for comparison.

## Recipe: making a crate's types `Snap`

1. Add `kiln_snap.workspace = true` to the crate's `[dependencies]`.
2. `use kiln_snap::Snap;` and add `Snap` to the `#[derive(...)]` of every
   plain data type: ids, nodes, enums, tables of ids. Build: every field
   type without `Snap` is an error at that field. For each, derive it on
   that type too, or decide the field is a cache (`#[snap(skip)]`) or needs
   a custom encoding (`#[snap(with = ...)]`).
3. Wrap ids in `NonZeroU32` where a `None` is common: `Option<Id>` is then
   as small as `Id`. Keep plain records free of `Box`, `String` and `Rc`
   where you can: a fixed-size record type takes the bulk path.
4. Find the roots (a type with locks, atomics, interior caches, closures,
   or fields that depend on the machine). Write their impls by hand in a
   `snap` submodule following [Hand-written roots](#hand-written-roots):
   destructure, classification table, validation, `within("Type.field")`
   on each field's error, a `schema` that registers a descriptor naming
   every encoded field (`Desc::field`) and every rebuilt one
   (`Desc::skip`), then recurses into each encoded field type.
5. Test (see `crates/kiln_source/tests/snap.rs`): build a populated value
   through the crate's public API; round-trip it; compare **every query**
   of the public API before and after; add the same content to both and
   compare the ids; compare `to_bytes_canonical` before and after;
   truncate at every byte and flip bits, asserting an error or a value
   whose queries do not panic.
6. Do not call the encoding from the compiler until the snapshot
   container (WP-1283 later phases) wires it in.

Gaps the recipe does not cover (met in `kiln_ast`, WP-1380):

- **A `hashbrown::HashTable` of ids** whose keys live in other arenas
  has no impl: its owner writes nothing for it and rebuilds it. When not
  every id is in the table (reserved slots, uninterned nodes), write the
  list of absent ids so that membership is exact after the rebuild.
- **`&'static str` fields** (names of compiled-in tables) have no impl:
  write the string and resolve it against the list of known names on
  read (`kiln_ast::slot::ColdMap`).
- **Records with ids as operands** are not range-checked by the derive;
  typed ids are, through the id spaces ("Id ranges"); ids packed into
  untyped integers need a per-kind walk the root has to write by hand.
- **Links to state outside the root** (source locations, ids into
  another crate's tables) are not checked by the encoding: the consumer
  must put the roots they point into in the same snapshot.

## Threat model

A snapshot is a cache the same compiler build wrote, read back on the
same machine: not an interchange format, and not a security boundary
against an adversary who can write the PCH file (whoever can write it
can also replace the compiler's inputs). What the decoders promise, in
order of strength:

1. **Never a panic, a hang or unbounded memory on any input.** Every
   read is bounds-checked, every length is checked against the bytes
   left before anything is allocated (`seq_len`), recursion follows the
   type structure only, sharing cannot form cycles, padding is zeros. A
   refusal is an error the driver turns into a replay (`-v` notes why).
2. **Damage is detected before use.** The PCH record's checksums (the
   image checksum and, with WP-1392, a per-block checksum) reject a file
   damaged in storage before any decoder runs: the raw-corruption fuzzer
   (`kiln_driver/tests/pch_snapshot_fuzz.rs`) sees only clean refusals.
3. **A well-checksummed but inconsistent snapshot is refused at decode,
   not at use** (the crafted model: damage behind recomputed checksums,
   standing for a writer bug). Each root validates its cross-table
   invariants (spec/source.md, spec/ast.md, spec/oven.md "Snapshots"),
   typed ids are range-checked per space (above), and the Oven's bytecode
   is verified (`kiln_oven/src/kbc/verify.rs`). The section head's
   lookup log has its directory indices checked (`LookupLog::validate`,
   spec/driver.md "Lookup-tolerant search path"). Known gaps, which can
   still reach a panic or a wrong result: ids in untyped `u32` slots of
   the AST and the other per-kind operand words of its nodes (enum
   tags, side-table indices; spec/ast.md "Snapshots"), links that form
   a cycle under a recursive walk (a stack overflow), counts used after
   the restore to size an allocation or a loop, the operands of the Oven's
   `Builtin` instruction past its result register (spec/oven.md
   "Snapshots"), the id-space sentinel `MAX_INDEX`
   where a real id is expected, and values the tables agree on but that
   no producer would write (a wrong type for an expression). The
   crafted fuzzer counts these outcomes (an internal compiler error, a
   `SIGABRT` abort, a hang) and prints them by site without failing,
   since about 2% of crafted mutants reach them wherever the layout puts
   the bytes. Any other signal fails, and every raw mutant must be a clean
   error. `KILN_PCH_FUZZ_STRICT=1` fails the gap as well: the gate for
   the work that closes it.
4. **Out of scope**: a malicious producer. A snapshot whose every check
   passes is trusted to be what the same build wrote; the compiler
   evaluates whatever constant-evaluation code and state it holds.

The inputs a snapshot depends on outside itself are checked by the
consumer: the schema hash and the identity (spec/driver.md), every file
the producer read (size and content, or with WP-1392 size and mtime),
and, when a lazily re-read prefix buffer no longer matches its recorded
length and hash, a fatal `drv_pch_file_modified` instead of compiling
against blank text (spec/source.md "Snapshots").

## Performance

Hand-run (`cargo run --release -p kiln_snap --example snap_bench`), best
of 7, single-threaded, aarch64 container (shared host), 2026-11-12:

- 1M 16-byte records: fast path encode ~2,000-2,300 MB/s, decode
  ~4,400-4,800 MB/s; general path (same records, `SIZE = None`) encode
  ~3,000-4,500 MB/s, decode ~4,100-4,300 MB/s. Encoding this size is
  bound by the allocator and page faults of the output buffer (the
  ranking flips with `MALLOC_MMAP_THRESHOLD_`), not by the per-item code.
- 100k short strings: encode ~2,000-3,600 MB/s, decode ~570 MB/s (one
  allocation per string).
- `FxHashMap<u32, u32>`, 100k entries: encode ~1,500-5,900 MB/s, decode
  ~1,200-2,000 MB/s.

## Tests

- `crates/kiln_snap/tests/snap.rs`: every derive shape, enum tags, fixed
  sizes and the flat bulk layout, standard impls, maps in both modes,
  sharing and its errors, truncation at every byte, bit flips, error
  paths, every schema-hash change, in the walk and in the constant;
  streaming reads (decoded as from a
  slice over every source chunking, short sources, trailing bytes);
  `derive_errors` doc tests.
- `crates/kiln_source/tests/snap.rs`: the pilots (see spec/source.md
  "Snapshots").
- `crates/kiln_diag/tests/snap.rs`, `crates/kiln_pp/tests/snap.rs`: the
  configured roots; the round trip of all four preprocessor-side roots at
  the end of a libstdc++ prefix, with the same main file read by the
  original and the restored front end (tokens, diagnostics, dependencies
  and canonical bytes equal); quiescence refusals; corruption.
- `crates/kiln_driver/src/snap_tests.rs`: the AST of real prefixes
  (`bench/pch` mid, gnu, lcx; a GCC 16 `<meta>` prefix) round-trips with
  the same `-fdump-ast`, canonical bytes and interning; truncations and
  bit flips of the mid snapshot never panic. The test-only hook is in
  `syntax_cmd.rs`.
- `crates/kiln_parse/tests/snap.rs`: `Sema` at the end of the pch bench
  prefixes and a `<meta>` prefix: encode, decode, encode; the original and
  the restored state continue a main file identically; quiescence;
  corruption.

## Open questions

1. **Order of hash maps.** Normal mode keeps iteration order in practice
   but not by guarantee. If a consumer turns out to depend on it, the
   choice is carrying the table layout (hashbrown's raw API) or making
   that consumer sort.
2. **Encoding speed for the fixed path.** The bulk path's encode writes
   item by item; a `bytemuck`-style memcpy needs `unsafe` or a layout
   guarantee the language does not give for `repr(Rust)`. Not needed at
   the measured rates.
