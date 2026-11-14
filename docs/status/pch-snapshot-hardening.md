# PCH snapshot hardening

WP-1396, 2026-11-13. The snapshot is still behind the hidden switches
`-fkiln-pch-snapshot=write|load`: no default change, no format promise.
The threat model (what a consumer promises for any bytes presented as a
PCH) is spec/snap.md "Threat model"; this note records what was checked
and what it costs.

**Summary.**
- The intact differential still holds: snapshot equals replay on every
  compared output, on the old cases and on three new ones that drive
  the Oven and reflection through restored state.
- Accidental damage (the raw model) is always caught: 40,000 raw
  mutants, no panic, abort or hang, and no mutant that changed a byte
  compiled differently.
- Crafted damage (behind recomputed checksums) is **not** yet always
  refused. In 40,000 crafted mutants, 718 ended in an internal compiler
  error (a caught panic, exit 70), 12 aborted (stack overflow or the
  4 GB memory cap), and 1 ran past 60 s. Almost all come from the known
  gap: the untyped operand words of AST nodes are not validated at
  decode (see "Known gaps"). The three crafted-mode panics of the
  default run are fixed, with reduced regressions.
- The added load cost on `gnu` is within the noise (about +0.6 ms for
  the bytecode verifier), under the 10 ms budget; on `lcx` about
  +4–9 ms.

## What is checked at decode

- **Padding** (WP-1387): the zero bytes of fixed-size records are
  checked on read (`kiln_snap` `skip_zeros`), so damage there is no
  longer silently ignored.
- **`#line` physical lines** (WP-1387): a restored `#line` directive's
  `phys_line` is checked when its lazy buffer is first read
  (`kiln_source` `check_directives`); a mismatch makes the buffer
  unavailable, as a changed file does (spec/source.md "Snapshots").
- **Typed ids** (spec/snap.md "Id ranges"): every typed AST id decodes
  by hand; an index above its space's `MAX_INDEX`, or a `QualType` with
  type index 0, is a decoding error. The reader keeps the largest index
  per id space (19 spaces), and after the restore the AST checks each
  against its arena, so an id past its table is refused before any use.
  Base-specifier lists and cast paths are checked too.
  - Existing checks before WP-1396: each root's cross-table invariants
    (spec/source.md, spec/ast.md §17, spec/oven.md "Snapshots").
  - Added: the per-space maxima (one compare per decoded id), the base
    and path lists.
- **Oven bytecode** (`kiln_oven/src/kbc/verify.rs`, its module docs
  list the invariants): one linear pass over the code. Valid opcodes on
  instruction boundaries ending at the end of the code; registers at
  most the owning function's `nregs`; jump targets on boundaries of the
  same function; function, global, type, trap, step-location and origin
  operands in range (`New`'s a heap origin); the `Unwind` pc, call-site
  pcs, function entries and the non-throwing cache in range; AST
  operands returned for the AST's range check.
- **Oven heap**: the pointers in static memory are not validated at
  decode. Like any pointer, each is checked at use (slot, generation,
  liveness, bounds), so a damaged one is a constant-evaluation error,
  not a crash. The static-memory hole list is range-checked.
- **Source text changed after the check**: a system header of the
  prefix (the record's check skips system files, as clang does) edited
  to the same size with its mtime kept passes the snapshot's default
  `mtime` file check (and so does any file with `-fno-validate-pch`).
  The lazy buffer's own hash then fails when the text is first read, and
  the compile ends with clang's fatal error `file '...' has been modified
  since the precompiled header '...' was built` instead of compiling
  against blank text (spec/pch.md "Consumer").
- **Lookup log** (WP-1403's part of the section head, read after the
  block checksum has passed): its counts are bounded by the bytes left,
  like every sequence. `LookupLog::validate` checks every directory
  index (results, includers, `after`, the search list) and the `<...>`
  start. A log that fails is a quiet refusal (`-v`), like any decoding
  error. It is not `-Wpch-snapshot-unused`, which stays for
  configuration refusals: a log that passes but finds another result.
  The re-resolution only joins names into paths and asks the file
  system, so hostile strings (empty, `..`, absolute, NUL, 32 KB deep)
  are files that are not there. Tests:
  - `lookup_log_damage` (`kiln_driver/tests/pch_snapshot_fuzz.rs`), with
    a consumer whose `-I` list differs, over a 476-byte log, every byte
    (`KILN_PCH_FUZZ_LOG_ALL=1`): crafted, 345 restored the same output,
    109 refused, 22 refused with the warning; raw, all 476 were clean
    errors (the block checksum); 35 hostile logs spliced in, 8 refused at
    validation (the index variants), 27 refused with the warning. No
    panic, signal or hang.
  - `hostile_logs_are_validated_and_repeat_without_panic` (kiln_pp, 3000
    random logs over a memory file system).
  - The main crafted fuzz now puts one mutant in eight inside the log.
    A short run on the merged decoder (`gnu`, 500 per model, release):
    crafted 113 restored, 375 refused, 1 clean error, 11 internal errors
    (the AST operand gap); raw 2 no-ops, 498 clean errors.

## Cost

Release builds, `gnu` (`bits/stdc++.h`) and `lcx` (libc++) prefixes, 5
interleaved runs each of the base (main at ec33ba521, with the streaming
decode of WP-1393) and this branch, `KILN_SNAP_TIMING=1`, load average
19–21. Medians, ms; "roots" is the restore of SourceManager through Sema
(`continue` excluded):

| | base | WP-1396 |
|---|---|---|
| gnu roots | 116.8 | 116.9 |
| gnu Ast / Oven | 79.1 / 9.3 | 76.7 / 9.9 |
| lcx roots (two sets) | 131.8, 125.3 | 135.9, 134.7 |
| lcx Ast / Oven (two sets) | 67.2 / 6.5, 64.8 / 5.7 | 71.4 / 7.2, 70.7 / 8.1 |

- On `gnu`, the budget's prefix, the added cost is within the noise
  (+0.1 ms on the roots; the verifier about +0.6 ms on the Oven).
- On `lcx` both sets show about +4–9 ms on the roots (Ast +4–6 ms,
  Oven +1–2 ms). Before the merge (base 7d2d4fc55, slice decode) the
  same comparison showed no AST difference and about +1 ms on the Oven,
  so the AST part may come from the id checks on the streaming path; it
  is at the edge of the noise at this load and worth a quiet-machine
  measurement.
- **Per-buffer hash**: the prefix buffers the compile reads are hashed
  when first read (byte-at-a-time FNV-1a, about 375 MB/s here). On
  `gnu` that is 63 buffers, 2.46 MB, about 6.6 ms; on `lcx` 89
  buffers, 1.73 MB, about 4.6 ms. The base already paid this inside
  `continue`. The fatal error adds no cost, and the `#line` check adds
  line starts only for buffers with directives (they are kept).
- **Derive cost** (WP-1387): a clean debug `cargo build --workspace`
  before the snapshot machinery (f42af4483) took 195.6 s wall, 106.4 s
  user, 1.76 GB max RSS; at 23c2312c7 251.4 s, 128.4 s, 1.90 GB (load
  22–31, so an upper bound that also holds three unrelated commits).
  `-Zmacro-stats`: `#[derive(Snap)]` expands to 22.7k lines in
  `kiln_ast` (213 uses), 3.7k in `kiln_oven` (46) and 8.8k in
  `kiln_sema` (100): about 5x the lines of `Debug` per use.

## Fuzzing

`crates/kiln_driver/tests/pch_snapshot_fuzz.rs`, two models:
- **raw**: the whole file is mutated and the checksums are left as
  written (accidental damage).
- **crafted**: the snapshot section is mutated and every checksum
  recomputed (`kiln_driver::pch_fuzz`), so the damage reaches the
  decoders (a writer bug).

Mutations: bit flips, byte runs, truncation (at every framing boundary
and at random), range swaps, and length-field edits.

**Targeted run** (CI, `framing_and_streamed_body_damage`, after the
merge with WP-1401's section-table search and WP-1393's streaming
decode): each of the 20 framing bytes of the section block, and 3 bytes
of the snapshot body that only the in-stream checksum sees, flipped
without recomputing checksums: all 23 refused or rejected cleanly.

**Default run** (CI, `snapshot_corruption_fuzz`, the built-in C++ and
C prefixes, 75 mutants per model each). What fails:
- **raw**: anything but a clean error, except a mutant whose bytes are
  the intact file's (a byte run that wrote what was there), which must
  restore the intact output. The checksums cover every byte, so this
  does not depend on the layout.
- **crafted**: the known gap (spec/snap.md "Threat model" 3) is counted
  and printed by compiler-source site, not failed: an internal compiler
  error through the crash handler, a `SIGABRT` abort (a stack overflow,
  or a failed allocation under the 4 GB cap), a hang (the compiler is
  killed at the 60 s limit and the stderr reader is not joined, so a
  hang cannot stall the test). About 2% of crafted mutants on a large
  prefix reach the gap wherever the layout puts the bytes. What still
  fails is independent of the layout: a signal other than `SIGABRT` (with
  `forbid(unsafe_code)`, a `SIGSEGV` or `SIGBUS` is not the known gap),
  and a harness error (the test panics). A crafted restore with another
  output is a valid outcome (a flipped constant is a valid constant) and
  passes, as before.
- **Strict mode**, `KILN_PCH_FUZZ_STRICT=1`: the gap fails too, which
  is the gate for the work that closes it.
- The log-confined crafted mutants (one in eight) and `lookup_log_damage`'s
  crafted byte flips follow the crafted rule. A flipped length or count
  can move the log's end, and so where the roots are decoded from: they
  can reach AST decoding differently from the intact file. The hostile
  logs `lookup_log_damage` splices in are well formed, so the roots
  start where they should and are the intact bytes; those stay strict
  (restored, or refused).

Seed independence (this tree, debug): seeds 1–20 and the default
0x5eed1396 all pass. 3 outcomes were tolerated in all, one each on
seeds 2 (`kiln_ast/src/ast.rs:491`), 9 (`decl.rs:2113`) and 13
(`lists.rs:70`). Strict mode fails 0 on the default seed and 1 on seed 2.

**Long run** (release, `KILN_PCH_FUZZ_FAST=1 KILN_PCH_FUZZ_ITERS=20000`,
4 threads, scratch under the system temp directory):

| | gnu crafted | gnu raw | lcx crafted | lcx raw |
|---|---|---|---|---|
| refused (replayed) | 14,237 | | 15,058 | |
| clean error | 97 | 19,937 | 82 | 19,945 |
| restored, output equal to replay | 5,244 | 63 | 4,492 | 55 |
| restored, other output | 25 | | 34 | |
| internal compiler error | 390 | | 328 | |
| abort (stack overflow or allocation failure) | 6 | | 6 | |
| hang (> 60 s) | 1 | | 0 | |

Wall time 41 min (gnu) and 38 min (lcx), on the tree merged with
WP-1392/1393/1394. "Restored, other output" counts crafted mutants that
restored and compiled to another result (a flipped constant is a valid
constant). The 118 raw "restored" mutants are byte runs that wrote the
bytes already there. A first run (before the merge, and with a harness
bug that counted fast mode's `-v` lines as a different output) gave the
same picture: 704 internal errors, 9 aborts (triaged: 6 stack overflows,
3 allocation failures under the 4 GB cap, one asking for 64 GiB) and 2
hangs.

The internal errors by message (first run, `gnu`; `lcx` is alike):
- a typed id read from an untyped word: index 0 (the reserved dummy)
  of `ListId`, `ExprId`, `DeclId` or `StmtId` (40), a zero `IdLists`
  word (14), or an id above its space (8);
- an enum tag in a node's `aux` that is no value of its enum: opcode,
  `CastKind`, `NonOdrUse`, `TraitKind`, `DeclName` tag (19);
- an index out of bounds of some table, from an untyped operand or an
  extra-table index (most of the rest).

Where they land (a 3000-mutant run on `gnu` with the merged decoder:
794 restored, 2137 refused, 13 clean errors, 3 other outputs, 52
internal errors, 1 hang; the 3000 raw mutants were all clean errors or
no-ops). Out-of-bounds reads in the arena accessors (index from an
untyped node operand) were 39 of the 52: `ExprArena::get`
(`kiln_ast/src/expr.rs:1624`, 14), `StmtArena::get` (`stmt.rs:574`, 8),
`IdLists` spans (`lists.rs:70`, 8), `DeclArena::get` (`decl.rs:2113`,
5), the type node table (`types/node.rs:180`, 2) and the constant-fold
statement walk (`kiln_codegen/src/fold.rs:550`, 2). Typed ids rebuilt
from an untyped word caused 6 (`expr.rs:1689`/`1690`, `lists.rs:97`,
`stmt.rs:766`/`788`), and an enum tag 1 (`expr.rs:1720`). One each
came from the type interner, the type context, the constant table,
`decl.rs:3194` and an attribute. The hang is a cycle in the member
links: `MemberIter::next` (`decl.rs:3253`) never ends, reached from the
Oven's record layout (`kiln_oven/src/ty.rs:1089`). The stack overflows
are the same kind of cycle in a recursive walk.

**Bugs found and fixed** (crafted model, each with a reduced
regression in `crates/kiln_ast/src/decl.rs` `snap_range_tests`):
- a declaration id past the arena panicked in `DeclArena::get`;
- an id outside its space;
- a base specifier past the arena panicked in `DeclArena::bases`.

## Differential

`scripts/pch-snapshot-diff.sh` gains three cases, each restored and
identical to replay:
- `oven`: recursion, loops, virtual calls, exceptions, unions, member
  pointers, `string_view`, `optional`, `variant` visits, a class
  template's static constant;
- `oven-limits`: the step limit, the depth limit, reads past the end,
  division by zero and null reads, with `-fconstexpr-steps` and
  `-fconstexpr-backtrace-limit` (the diagnostics and notes must match);
- `meta-oven`: `define_aggregate`, `define_static_array`,
  `define_static_string`, `template for` over enumerators,
  `substitute`, and size queries on prefix entities.

The whole differential (`scripts/pch-snapshot-diff.sh`, 18 cases after
the merge with WP-1392/1393/1394): every case restored, snapshot equal to
replay. `cargo xtask test --pch-diff snapshot` over `cpp.include` and
`module.interface`: 89 passed, 0 failed, 138 skipped.

## Known gaps

- **AST node operands** (the bulk of the long run's failures): the
  `aux`, `a` and `b` words of expression, statement and declaration
  nodes, and the indices into their side tables, mean different things
  per node kind and are not validated at decode. Closing this needs a
  per-kind operand rule for every node kind, checked in one pass after
  the restore, like the bytecode verifier. The reduced regression is
  `binary_operator_without_an_opcode_is_refused` (`kiln_ast` `decl.rs`,
  ignored until then).
- **Cycles and depth**: a crafted link can make a recursive walk over
  restored structure overflow the stack (6 aborts in the first long
  run). Acyclicity of the links a recursive walk follows is not checked.
- **Allocation from a crafted count** (3, one asking for 64 GiB) and
  **non-termination** (2): a count or link taken from the snapshot sizes
  an allocation or a loop after the restore. The decoders bound their
  own allocations by the input size; later users do not.
- `Builtin` operands past its result register are not verified: their
  meaning depends on the builtin kind.
- Ids in untyped `u32` slots of the AST are not range-checked.
- An id-space sentinel `MAX_INDEX` where a real id is expected passes
  the range check.
- `DmsId` indexes the Oven's table and is not bounded by the AST.
- Values the tables agree on but no producer would write (a wrong type
  for an expression) are not detected. The threat model's promise for
  them is only "no crash in the checked paths".
