# PCH snapshot prototype (P3): go/no-go evidence

WP-1383, 2026-11-13. This is a prototype behind the hidden switches
`-fkiln-pch-snapshot=write|load`. There is no format promise and no
default change. The design study's go rule (study §8) is: the
differences are few, they can be classified as order-only, and a `gnu`
load takes under 0.3 s.

**Verdict: GO.**
- Snapshot equals replay byte for byte on every compiled output. That
  covers 4 prefixes plus a diagnostics case, 86 corpus tests that were
  restored, and the 3 `pch:` corpus tests.
- No class (i) differences (raw `Loc` order), no class (ii) (hash-map
  order) and no class (iii) (classification). There is one policy
  difference (class iv): the prefix's own warnings are not shown again.
- The restore of the `gnu` roots takes 58 ms. With today's unoptimised
  validation it takes 0.22 s.

## What was built

- **Producer.** With `=write`, `pch::write_pch` runs a second job. Its
  main file is an empty `<stdin>`, and its `-include`s are the
  producer's own followed by the header.
  - The job is parsed with `kiln_parse::parse_prefix` and must be
    `ready`. Then `end_main_file` runs, and the roots are written in one
    stream: SourceManager, `Arc<Interner>`, DiagEngine, Preprocessor,
    Sema (Ast, Oven, fields). See `SyntaxJob::snapshot`
    (`kiln_driver/src/syntax_cmd.rs`).
  - The stream goes into `SECTION_SNAPSHOT` (4) of the record's section
    block, next to the image. Replay and image consumers read the same
    file.
  - If the prefix is not at rest, or has errors, or is a C header, the
    PCH gets no section, and `-v` prints a note with the reason.
- **Consumer.** With `=load`, `pch::consumer` (a compile, not
  `-E`/`-M`/`-verify`/`=check`) looks up the section and checks three
  things:
  - the schema hash of all five roots;
  - the identity: the kiln version and sha, the cwd, and the command
    line in order, minus the excluded options;
  - every recorded file, by size and checksum.

  It then restores in the order of spec/snap.md "Whole-compiler restore
  order" (new section). `SyntaxJob`/`CodegenJob::run_restored` continue
  with `continue_preprocessed` and then the normal codegen path. Any
  mismatch or decode error means a silent replay, with a `-v` note.
- **Options the identity excludes:**
  - inputs and outputs, and the action: `-o`, `-x`, `-c`, `-S`, `-E`,
    `-fsyntax-only`, `-emit-llvm`, `-emit-pch`;
  - `-include`/`--include` (the consumer's extras after the PCH are
    entered by `enter_builtin_includes`), and `-include-pch`;
  - dependency output: `-MD`, `-MMD`, `-MP`, `-MG`, `-MF`, `-MT`, `-MQ`,
    `-MJ`;
  - `-v`, `-g*`, `-fdump-ast*`, `-ferror-limit=`;
  - `-f[no-]color-diagnostics`, `-fdiagnostics-color*`;
  - `-f[no-]kiln-pch-image*`, `-f[no-]kiln-pch-snapshot*`;
  - the same options behind `-Xclang`.
- **The P2 configuration lists are covered.** Every field classified
  "configuration" comes from a command-line option, and every such
  option stays in the identity. That holds for DiagEngine (`policy`
  from `-W`, and `clang_compat`), Preprocessor (`opts`, `lex_opts`,
  features, `modules`), Oven (`budget`, `backtrace_limit`, memo size,
  `clang_compat`, `cxx_year`, `hooks`) and Sema (the 30 fields in
  `kiln_sema/src/snap.rs`).
  - The one exception is `-ferror-limit=`, which is excluded. A prefix
    with errors gets no snapshot, so the limit cannot have changed the
    prefix's state.
  - The memo capacity is a configuration field and stays the consumer's.
- **Not covered by the identity:** environment inputs such as `CPATH`
  and GCC installation auto-detection (a follow-up, W1).
- **Diagnostics policy.** The restored DiagEngine carries the prefix's
  state. The main file's diagnostics, notes into the prefix, and the
  `#pragma clang diagnostic` state match replay exactly.

## Go/no-go table

The measurements use a release build on an aarch64 container shared
with other agents, with load average 15–21 throughout. Each figure is
the median of 5 runs, for `-c` at `-O0`.
- "empty" is `int empty_tu;`.
- "main" is the case's main file from `scripts/pch-snapshot-diff/`
  (about 30–60 lines; it instantiates `vector`, `map`, `sort` and
  `format`, uses ranges and lambdas, and the `meta` case uses
  reflection).
- **Validate** is the section search plus the checksum, the identity
  and the file check.
- **Roots** is the decode of SourceManager through Sema.
- **Load** is validate plus roots, the time the go rule measures.

| prefix | textual empty | replay empty | snapshot empty | validate | roots | load | textual main | replay main | snapshot main |
|---|---|---|---|---|---|---|---|---|---|
| gnu `<bits/stdc++.h>` | 1.44 s | 1.96 s | **0.63 s** | 160 ms | 58 ms | **0.22 s** | 1.87 s | 2.02 s | 1.08 s |
| mid (4 headers) | 0.41 s | 0.46 s | 0.22 s | 65 ms | 25 ms | 0.09 s | 0.50 s | 0.54 s | 0.35 s |
| lcx (all libc++ 19) | 1.31 s | 1.38 s | 0.53 s | 160 ms | 57 ms | 0.22 s | 1.96 s | 2.28 s | 1.21 s |
| meta (GCC 16 `<meta>`) | 0.94 s | 0.94 s | 0.58 s | 205 ms | 28 ms | 0.23 s | 0.84 s | 0.77 s | 0.38 s |

**Peak RSS (empty).** RSS is higher with the snapshot because the whole
section stays in memory while it is decoded (W6, streaming decode;
done by WP-1393, below).

| prefix | textual | replay | snapshot |
|---|---|---|---|
| gnu | 93 MB | 94 MB | 117 MB |
| mid | 58 MB | 58 MB | 58 MB |
| lcx | 90 MB | 90 MB | 121 MB |
| meta | 58 MB | 58 MB | 68 MB |

**Producer cost and PCH size.**

| prefix | write | write with snapshot | PCH | PCH with snapshot |
|---|---|---|---|---|
| gnu | 1.38 s | 2.93 s | 84 KB | 36.6 MB |
| mid | 0.38 s | 0.73 s | 32 KB | 8.8 MB |
| lcx | 1.29 s | 2.67 s | 137 KB | 41.2 MB |
| meta | 0.97 s | 1.85 s | 74 KB | 17.6 MB |

- The second pass reparses the prefix: 1.13 s for gnu. The encode takes
  47 ms.
- **Per-root split** (`KILN_SNAP_TIMING=1`, gnu, empty main):
  - SourceManager 4.8 ms, Interner 3.4 ms, DiagEngine 0.05 ms;
  - Preprocessor 6.2 ms, Ast 35.9 ms (intern-table rebuilds 5.6 ms of
    that), Oven 3.5 ms, Sema 3.7 ms;
  - then `continue` 193 ms.
  - The validation took 96 ms for the sections (the FNV checksum is byte
    at a time over 36 MB), 20–50 ms for the identity (the schema hash is
    recomputed on every load) and 27–55 ms for the file check (508 files
    read and hashed).
- **`continue` on an empty main** costs 193 ms for gnu and 61 ms for lcx.
  That is end-of-TU work over the prefix's declarations
  (`kiln_parse/src/lib.rs:502` `p.finish()`, `:507` `s.memory()`), and
  replay and textual pay it too. The Oven restore is 2–7 ms, and the
  `meta` main's `continue` is 74 ms against 0.77 s for replay: nothing
  is evaluated again after a restore.
- **clang 19:** the harness has no snapshot row, so there is none here
  (bench/pch already has clang's PCH rows).

## Differential

`scripts/pch-snapshot-diff.sh <kiln> <out>` compiles each case three
ways (plain `-include` as the judge, replay, snapshot). It compares
snapshot with replay on the exit status, the diagnostics, `-fdump-ast=locs`
of the main file, IR at `-O0`, IR with `-g`, and the `-MD` list. The
`snapshot-timing` line confirms that every run of every case was restored.

| case | snapshot vs replay | replay vs plain |
|---|---|---|
| gnu, mid, lcx, meta | identical (all 6) | `.d` only (a replay quirk, see below) |
| diag (prefix with `#warning` and a deprecated use; main with 4 errors, notes into prefix templates, `-Wall`) | diagnostics only: class (iv) | `.d` only |

- **Coverage of the `meta` main.** It exercises the reflection bridge on
  restored state:
  - `identifier_of`, `is_type`, `members_of`,
    `nonstatic_data_members_of` and `enumerators_of` on prefix entities;
  - an `info` constant from the prefix, and a consteval function and a
    template from the prefix used over reflections;
  - `define_aggregate` of an incomplete prefix class and of a main-file
    class (DmsTable), then aggregate initialisation and IR use of both.

  All of it was identical. Nothing pointed at `meta.info_ty`, the dms
  plan or `tu_arena_peak`.
- **Corpus slice.** `cargo xtask test --pch-diff snapshot` over
  `cpp.include` and `module.interface` gave 89 passed, 0 failed and 138
  skipped (by the same rules as the other modes).
  - 86 tests were restored and 3 replayed. The 3 are
    `module.interface/carried-*reflections*` and
    `carried-alias-value-arg`: their consumer adds `-fmodule-file=`
    flags that the harness producer lacks, so the identity differs.
    This is harness shape, and they pass by replay.
- **The 3 `pch:` corpus tests** (`pch-run`, `pch-redefinition`,
  `pch-consumer`) were restored by hand, and their exit status,
  diagnostics and IR are identical to replay.

**Differences by class:**
- (i) raw `Loc` order: **0**. The main file's locations follow the
  prefix's in a snapshot but precede them in replay, and no output
  showed it.
- (ii) hash-map iteration order: **0**.
- (iii) classification mistake: **0**, so no fix commits.
- (iv) producer shape: **1**, and it is a policy question. The prefix's
  own diagnostics are not shown again. Replay shows them (`prefix.h:30:2:
  warning: "the prefix warns"` and a deprecation inside the prefix);
  a snapshot shows them only once, when the PCH is built, as clang's
  PCH does. So the summary line differs: `3 warnings and 4 errors
  generated.` against `1 warning and 4 errors`. The main file's 4 errors,
  its notes into prefix templates (`needs_member`, the `Checked`
  `static_assert`) and their locations are identical.
- (v) other: **0**.
- **Not a snapshot difference:** replay's own `-MD` list differs from
  plain's. Replay lists `main.cpp` last instead of first, because
  `pch.deps` is seeded before the main file. Snapshot equals replay
  there.

**Quiescence failures:** none on the 4 prefixes, the corpus slice or
the `pch:` tests. Every prefix ended at rest.

## Unknowns

- No `-verify` consumer has been tried, because it is excluded from the
  snapshot path. No PCH chain or module-unit consumer has been tried
  either.
- Prefixes ending in `#pragma push_macro`, or in an open `#pragma
  clang diagnostic push`, were not exercised beyond the P2 unit tests.
- The corpus run is a slice: the libc++ suite was not run with
  snapshots.
- The timings were taken under load 15–21, so absolute numbers carry
  about ±20%. The ratios held across runs.

## Corrected product estimate (study §8)

- **W1** (driver) **shrinks** to about 0.5 WP. The switches, the
  framing, the fallback and the restore plumbing exist. What remains:
  - a real fingerprint: order-insensitive where the order cannot
    matter, environment inputs, and the GCC installation;
  - mtime-first file validation;
  - stable options and notes.
- **W2** (raw `Loc` order) **shrinks** to a watch item: 0 divergences
  across about 95 compiles.
- **W3** (Fx-map order) **shrinks** to a watch item: 0 divergences.
- **W4** (diagnostics policy) stays at 1 WP and needs an owner decision.
  The options are to keep clang's behaviour (no prefix warnings shown
  again, as now) or to store the prefix's diagnostics and print them
  again. `-verify` must also be designed.
- **W5** (reflection and Oven) **shrinks** to tests only. Restored
  `define_aggregate` and the reflection queries work unchanged.
- **W6** (perf) **grows** a little.
  - The validation costs 3× the restore: a word-wise checksum, the schema
    hash as a build constant, and mtime file checks are worth about
    150 ms on gnu.
  - The section is held whole (+24–35 MB RSS). Done by WP-1393.
  - The producer's second parse doubles the write time; reusing the
    first parse would mean making the header job stop at the prefix.
    Done by WP-1393 for headers whose two shapes cannot differ.
- **W7** (corpus and gates) is unchanged. It also needs a harness
  producer that mirrors the consumer's flags, for module tests.
- **W8** (spec) is unchanged; the restore order is now written down in
  spec/snap.md.
- **W9** is unchanged.

The net is about 6 product WPs instead of 8.

## Validation after WP-1392

`<bits/stdc++.h>` (gnu), empty main, `-c`, release build, 5 runs each,
`-v` timing lines:

- Before (load average 16.3/15.0): section block 94-96 ms, identity with
  the schema hash about 1.9 ms, file check 20-33 ms. Wall 437-530 ms.
- After (load average 18.7/17.5): section block 4.2-4.7 ms (word-wise
  `block_checksum`), schema hash 1.7-2.1 ms (once per process), identity
  1.0-1.4 ms, file check 1.0-1.2 ms (mtime first): about 8.5 ms. Wall
  320-348 ms.

## WP-1393: one producer pass, streaming decode (2026-11-13)

Still behind `-fkiln-pch-snapshot=write|load`; the default is unchanged
and replay stays the fallback. The bench rows are in
[pch.md](pch.md), regions `wp-1393-before`, `wp-1393-after` and
`wp-1393-merged` (after the merge with WP-1392 and WP-1394); the first two ran
40 minutes apart under different load (16 and 20), so the table below is
an interleaved A/B of the old and the new binary instead: both release
builds, median of 7, each round runs every row with both binaries, load
17–20, wall s / peak RSS MB, `-O0`.

| prefix | build | syntax main | `-c` main | syntax empty | `-c` empty | replay empty |
|---|---|---|---|---|---|---|
| gnu before | 2.79 / 139 | 0.51 / 115 | 0.63 / 117 | 0.50 / 114 | 0.54 / 115 | 1.45 / 86 |
| gnu after | **1.60** / 124 | 0.49 / **84** | 0.62 / 86 | 0.45 / **81** | 0.51 / 82 | 1.46 / 87 |
| lcx before | 2.37 / 155 | 0.41 / 120 | 0.55 / 121 | 0.38 / 117 | 0.46 / 117 | 1.22 / 86 |
| lcx after | **1.36** / 127 | 0.39 / **87** | 0.53 / 87 | 0.34 / **87** | 0.43 / 87 | 1.20 / 86 |
| mid before | 0.61 / 52 | 0.15 / 43 | 0.24 / 53 | 0.12 / 41 | 0.13 / 47 | 0.33 / 33 |
| mid after | **0.35** / 49 | 0.13 / **36** | 0.21 / 53 | 0.11 / **35** | 0.13 / 47 | 0.32 / 33 |
| meta before | 1.32 / 59 | 0.31 / 48 | 0.41 / 53 | 0.28 / 46 | 0.32 / 48 | 0.60 / 37 |
| meta after | **0.79** / 56 | 0.33 / **39** | 0.42 / 53 | 0.29 / **38** | 0.34 / 47 | 0.62 / 37 |

PCH sizes are unchanged: gnu 36.6 MB, lcx 41.2 MB, mid 8.8 MB, meta
10.9 MB. Replay rows are the control (no change expected).

**After merging WP-1392 and WP-1394** (block checksum, mtime file
check, C headers, diagnostics policy), the same binary interleaved with
textual kiln, replay and clang 19 (`-include-pch`), median of 7, load
21.8–24.7, wall s / peak RSS MB, `-O0`. This is the final table.

| prefix | build | syntax main | syntax empty | `-c` main | replay main | textual main | clang build | clang PCH main | clang PCH empty | clang textual main |
|---|---|---|---|---|---|---|---|---|---|---|
| gnu | 1.40 / 124 | **0.35** / 82 | 0.31 / 82 | 0.47 / 85 | 1.34 / 89 | 1.32 / 88 | 1.52 / 273 | 0.38 / 162 | 0.35 / 157 | 1.40 / 233 |
| lcx | 1.21 / 127 | **0.24** / 83 | 0.20 / 81 | 0.37 / 84 | 1.16 / 89 | 1.16 / 88 | 1.51 / 258 | 0.20 / 146 | 0.13 / 134 | 1.25 / 213 |
| mid | 0.38 / 48 | **0.09** / 35 | 0.08 / 34 | 0.18 / 50 | 0.35 / 34 | 0.33 / 32 | 0.34 / 134 | 0.09 / 107 | 0.04 / 98 | 0.29 / 122 |
| meta | 0.56 / 56 | **0.13** / 40 | 0.09 / 39 | 0.21 / 50 | 0.48 / 38 | 0.45 / 37 | — | — | — | — |

(clang 19 cannot compile GCC 16's `<meta>`.)

**What changed.**
- **The producer parses once** when it can (spec/snap.md "The
  producer's single pass"). The snapshot pass (the header as an
  `-include` of an empty main) runs first; when nothing a main file
  alone gets could differ, its end of TU stands in for the header pass,
  so `syntax_only` over the header as main file is skipped. Build time
  falls 40–45% on every prefix. What differs between the two shapes,
  and how each is handled:
  - the SourceManager layout and include chain, `__BASE_FILE__`,
    `__INCLUDE_LEVEL__`, `pp.main_file`: the PCH record and the
    snapshot are written from the snapshot pass in both modes; a
    header (or any file it includes) that spells `__INCLUDE_LEVEL__` or
    `__BASE_FILE__` takes two passes;
  - the `-MD` order (the main file first): the header is moved to the
    front of the dependency list;
  - main-file-only diagnostics: `#include_next` in the primary file,
    `#pragma GCC system_header` in the main file, and the unused-declaration warnings
    (`-Wunused-variable`, `-Wunused-const-variable`,
    `-Wunneeded-internal-declaration`, unused variable templates) that
    apply to main-file declarations only: two passes when the header has
    any of them (an unused-declaration warning that is enabled, unless
    the header declares nothing);
  - any diagnostic at all from the snapshot pass, `-v`, `-verify`, a
    time trace, dumps, reports, `-MG` or an image mode: two passes, as
    before. A header with errors therefore costs one extra pass
    (+0.1–0.2 s) before the fallback; that is the price of keeping the
    header pass's own diagnostics and exit status.
  - `KILN_PCH_SNAPSHOT_PASSES=2` forces the old two passes; the test
    `kiln_driver/tests/pch_snapshot_producer.rs` compares both modes on
    exit status, diagnostics, the `-MD` file and the PCH record, and the
    consumer IR. 26 hand cases (the four prefixes with and without
    `-Wall -Wextra`, the diag case, nine small headers with and without
    `-Wall`) gave identical output. (`#pragma once` in a header compiled
    as main is already silent, so it does not differ.)
- **The decode streams.** The consumer no longer reads the section into
  memory: `open` reads it from the file through `kiln_snap::Reader::
  streaming`, a 1 MiB window that drops decoded bytes (no copy of the
  whole section, no zero fill). Peak RSS falls 30–33 MB on gnu and lcx
  and 6–8 MB on mid and meta, to within 3 MB of replay's (gnu 81 vs 87,
  lcx 87 vs 86, mid 35 vs 33, meta 38 vs 37). The PCH's size and mtime
  are checked again at open, so a file changed after the check falls
  back to replay. A PCH read from memory (no file) keeps the slice
  reader.
- **The type store decodes arena by arena** instead of through its
  derived `get`, which measured three times slower (14 ms against 5 ms
  for gnu's 5 MB store).
- **No copies of the section block on load.** `pch::load` keeps the
  file buffer as the image (`Record::parse_owned`) and
  `bmi::section_ranges` locates the sections in place; the block is
  checksummed once (WP-1392's `bmi::block_checksum`).
- **The streamed block is verified during the stream.** The streaming
  reader re-reads the block from the file through `bmi::BlockChecksum`
  (the prefix sections are read into the sum and dropped); the sum is
  compared after the last root is decoded and before anything restored
  is used, so a mismatch refuses the snapshot and falls back to replay.
- **C headers take two passes**: C's main-file-only diagnostics are not
  in the single-pass list (`single_pass_allowed`).

**Per-root split** (`KILN_SNAP_TIMING=1`, gnu, empty main, before →
after): SourceManager 5.3 → 8 ms, Interner 4.4 → 3.9 ms, Preprocessor 9
→ 4–7 ms, **Ast 36–38 → 42 ms** (with the streaming reads, which now
fall inside the decode timers: about 8 ms of file reading moved in from
the section search), Oven 3.3 → 5 ms, Sema 3.5 → 5 ms (load 20–30; ±20%).
Ast by field, after: types 16.7 ms (7.9 MB; store 5.1 ms, intern-table
rebuild 5.6 ms, template arguments 3.8 ms), decls 5.7 ms (7.2 MB), exprs
5.1 ms (7.0 MB), templates 3.2 ms, lists 1.9 ms, lookup 1.3 ms, stmts
0.8 ms, redecls 0.7 ms, the rest under 0.3 ms. Decode runs at 0.5–1.5
GB/s per field; the rebuilds of the intern tables (6–7 ms) are the
largest single item left, and are what keeps hash order out of the
format.

**End of TU on an empty main.** `continue` after a gnu restore costs
190–220 ms. Of that, 191.5 ms is `perform_pending_instantiations`
([temp.point]/7: the implicit instantiations the prefix requested, done
at the end of the translation unit); the walks after it (unused
declarations, deferred checks) take 2–4 ms. Textual kiln pays the same
(gnu `-fsyntax-only` with an empty main: 1.41 s, of which the same
instantiations are about 0.2 s); clang 19 pays 278 ms for them textually
and its PCH consumer, which re-runs them, takes 0.53 s on the same empty
main. Nothing here is removable without changing which instantiations a
consumer performs, so this WP removed nothing. The lever is clang's
`-fpch-instantiate-templates`: instantiate in the producer and carry the
results in the snapshot. It needs an owner decision (it changes where
instantiation diagnostics appear).

**Gap to clang 19's PCH** (final table, `-fsyntax-only`):
- gnu: main 0.35 s against clang's 0.38 s, empty 0.31 s against 0.35 s,
  at 0.51x clang's memory: kiln is ahead.
- lcx: main 0.24 s against 0.20 s, empty 0.20 s against 0.13 s: clang
  is ahead by 40–70 ms, at 0.57x of its memory.
- mid: main level (0.09 s), empty 0.08 s against 0.04 s.
- meta: 0.13 s against 0.48 s for replay and 0.45 s textual.
- What remains of a gnu consumer (`KILN_SNAP_TIMING=1`, after the
  merge): validation before any decode is about 27 ms (sections 2.5 ms,
  schema hash 21 ms, identity 2 ms, mtime file check 1.2 ms); restore
  about 75 ms (Ast 47–51 ms); end of TU 209–247 ms, the instantiations
  clang pays too.

**Follow-ups.**
- The identity's schema hash is recomputed on every load (about 21 ms
  on gnu, `kiln_driver/src/pch_snapshot.rs` `identity`); a build-time
  constant would remove it.
- Pre-instantiation in the producer (above).
