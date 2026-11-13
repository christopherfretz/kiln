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
section stays in memory while it is decoded (W6, streaming decode).

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
  - The section is held whole (+24–35 MB RSS).
  - The producer's second parse doubles the write time; reusing the
    first parse would mean making the header job stop at the prefix.
- **W7** (corpus and gates) is unchanged. It also needs a harness
  producer that mirrors the consumer's flags, for module tests.
- **W8** (spec) is unchanged; the restore order is now written down in
  spec/snap.md.
- **W9** is unchanged.

The net is about 6 product WPs instead of 8.
