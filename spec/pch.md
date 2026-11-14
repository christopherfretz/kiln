# Precompiled headers

This is the contract of a kiln precompiled header (PCH): what the file
holds, what a producer writes, and what a consumer does with it. The
command-line surface that makes a compile a producer or a consumer
(`-x c++-header`, `-emit-pch`, `-include-pch`, the `-include` probe,
`-include-pch <dir>`, `-fno-validate-pch`, CMake's command lines) is in
spec/driver.md "Precompiled headers". The snapshot's encoding is in
spec/snap.md. Every statement below names the test that checks it;
paths are relative to `crates/kiln_driver/` unless they say otherwise.

The rule a consumer follows is [cpp.include]/2: a compile with
`-include-pch p.pch` behaves as if the header `p.pch` was built from
were `-include`d. kiln reaches that state in one of two ways:

- **restore**: it decodes the PCH's *snapshot*, the whole front-end state
  at the end of the header, and goes on with the main file;
- **replay**: it processes the header again, as if `-include`d.

A restore is the default. A replay is the fallback, and is always
correct: every reason not to restore (below, "Consumer") ends in a
replay or in an error that says the PCH must be rebuilt, never in a
compile against a state that differs from the header's.

**Trust.** A PCH is a trusted build artifact, tied to the exact kiln
build that wrote it and to its input files. kiln checks that it is the
file it wrote (checksums), that it was written by this build (the
record's `kiln` line, the snapshot's format version and schema hash),
that the configuration is the one it was built with (the identity),
and that its inputs have not changed (the file check). Nothing in it is
checked beyond that: a file crafted to pass those checks is not
defended against beyond what spec/snap.md "Threat model" says of any
decode (bounded counts, no panic, no unbounded allocation). Do not
consume a PCH from a source you would not take a compiler from.

## File layout

A kiln PCH is a text record followed by a binary section block:

```
KILNPCH 2 ... end <checksum>\n        the record (UTF-8 text)
<section block>                       bmi::write_sections + checksum
```

### The record (`KILNPCH 2`)

The record is UTF-8 text, one field per line. Values are tab-separated,
with `\\`, `\t`, `\n` and `\r` escaped. It is written atomically (a
temporary file, then a rename). Lines, in order:

```
KILNPCH 2                               magic and format version
kiln <version> <commit|-> -fclang-compat=<N>
cwd <producer's working directory>
main <the header, as spelled>
include <a -include of the producer>    (0 or more, in order)
opt b\t<option>\t0|1                    a language option (bool)
opt v\t<option>\t<value>                a language option (value)
target <triple>
macro D\t<name>\t<definition>           -D/-U of the producer, in order
macro U\t<name>
file <size>\t<mtime ns>\t<fnv1a64 hex>\t<0|1 system>\t<spelled>\t<absolute>
image <length>\t<block checksum hex>    (with a section block)
end <fnv1a64 hex of every byte before this line>
<the section block: <length> bytes>
```

The text runs up to and including the `end` line; only the text has to
be UTF-8. Without an `image` line, any byte after `end` makes the record
corrupt ("trailing bytes"). With one, the bytes after `end` must be
exactly the section block, with that length and checksum. (The keyword
`image` is historical: the line frames the section block.) Tests:
`src/pch.rs` `record_round_trip`; `tests/pch_snapshot_fuzz.rs`
`framing_and_streamed_body_damage`.

The `file` lines list every file the producer read (the dependency list
`-MD` writes), with each file's size, modification time (nanoseconds
since the Unix epoch, taken before the file is read; 0 when the file
system has none) and FNV-1a 64 content hash. An unknown line, a missing
field or a bad checksum makes the record corrupt. Another `KILNPCH`
version is "older" (1, what kiln 1.0 and 1.1 wrote before the
modification time and the block checksum) or "newer"
(`src/pch.rs` `record_round_trip`; `tests/pch_snapshot.rs`
`cross_version`). The language options are those of the module-file
fingerprint (spec/modules.md §11.4), named as clang names them: the
standard levels, `char8_t keyword`, GNU extensions, exceptions, RTTI,
freestanding, signed char, sized deallocation, C++26 reflection, the
reflection mangling, and the optimization level.

A consumer reads the record as a stream (WP-1401): the text whole, then
the section block in pieces through its checksum and the block's own,
leaving the block in the file. The checks, their order and their
verdicts are those of a read of the whole file (`src/pch.rs`
`streamed_record_reads_as_a_whole_one`).

### The section block

`bmi::write_sections` (crates/kiln_pp/src/bmi.rs), the module files'
kind-2 section block without the capsule:

```
u32 count                               little-endian, at least 1
count x { u32 tag; u32 length; length bytes }   by strictly increasing tag
u64 block checksum                      bmi::block_checksum of what precedes
```

`bmi::block_checksum` is word-wise (four lanes of 8-byte words, the
tail through FNV-1a), so it runs at memory speed over a section of tens
of megabytes; a change of any byte or of the length changes it.
`bmi::section_ranges` checks the checksum and returns each section's
tag and byte range; `bmi::section_table` reads the same table from a
block whose checksum was checked while it was streamed, reading only the
count and each tag and length. Both refuse a truncated block, a count
or length larger than the bytes left, a zero count, a tag of 0 or above
`bmi::SECTION_MAX`, and tags out of order. A damaged block is the
error `malformed or corrupted precompiled file` (never a guess):
`tests/pch_decls.rs` `damaged_section_block_is_an_error`,
`tests/pch_snapshot_fuzz.rs` `framing_and_streamed_body_damage`.

A kiln 1.2 PCH has one section, tag 4 (`bmi::SECTION_SNAPSHOT`). Tags
1 to 3 are the module files' (spec/modules.md §11.11, §11.13); a PCH
block holding them is read and they are ignored.

### The snapshot section

```
"KILNSNAP"                              8 bytes, the magic
u32 FORMAT_VERSION                      little-endian (now 3)
kiln_snap stream: u64 SCHEMA_HASH, identity, lookup log
kiln_snap stream: the roots
```

- The magic and `pch_snapshot::FORMAT_VERSION` are checked before
  anything is decoded: a section shorter than 12 bytes is `its section
  is truncated (no header)`, another magic `its section is not a kiln
  snapshot (bad magic)`, another version `its section has format version
  N, expected 3` (`src/pch_snapshot.rs` `tests::section_header`;
  `tests/pch_snapshot.rs` `section_header`, `cross_version`).
- The head's stream holds `pch_snapshot::SCHEMA_HASH` (u64), the
  identity (a list of `(component, text)` pairs, "Identity" below) and
  the prefix's lookup log (`kiln_pp::LookupLog`, "Lookup-tolerant search
  path" below). It is a stream of its own, so the roots' sharing table
  starts with the roots.
- The roots' stream holds, in this order, the source manager, the
  interner, the diagnostics engine, the preprocessor and `Sema` with its
  AST and Oven (spec/snap.md "Whole-compiler restore order"); a restore
  keeps every id unchanged.
- The section has no checksum of its own: the block checksum covers it.
  A consumer that streams the block decodes the roots from the file
  (spec/snap.md "Streaming decode").

Version 1 (WP-1392) had no lookup log; version 2 (WP-1403) added it;
version 3 (WP-1411) is a new diagnostic id, which changes `DiagId`'s
schema hash.

### SCHEMA_HASH and SHAPE_HASH

`Snap::SCHEMA_HASH` is a compile-time constant per type (spec/snap.md
"Schema hash"): a `SchemaHash` fold of the type's kind and name, each
field's name with its type's `SCHEMA_HASH`, each variant's name and
discriminant, and notes. It changes when any type the type reaches adds,
removes, renames, reorders or retypes a field, changes a variant, or
changes an attribute that changes the encoding. It is derived from
names only: it is the same on every host that builds the same sources
(no snapshotted type has a field under a `cfg` that differs between
aarch64 and x86-64 Linux).

`Snap::SHAPE_HASH` is a type's own items without what its fields reach
(a derived type's fields by their spelled types, a container's elements
by their `SHAPE_HASH`). It is what a field that closes a cycle of types
folds in place of a `SCHEMA_HASH` that would depend on itself; every
type that holds no other type has `SHAPE_HASH == SCHEMA_HASH`.

`pch_snapshot::SCHEMA_HASH` folds the five roots: `SourceManager` and
`Arc<Interner>` by their `SCHEMA_HASH`, the diagnostics engine, the
preprocessor and `Sema` by their `SNAP_SCHEMA_HASH` (configured roots,
which fold their registered fields' types). The diagnostics engine's
constant folds `DiagId` with the name of every diagnostic id, so adding,
removing or renaming a diagnostic changes it. `src/pch_snapshot.rs`
`tests::schema_constants_cover_every_type` checks that every type a
descriptor walk of the roots reaches has a constant, and that two
different descriptors never share one. A section with another schema
hash is refused, never misread: `written by another build (schema
hash)` (`tests/pch_snapshot.rs` `cross_version`).

### Format versioning

`crates/kiln_driver/pch-snapshot-format.txt` records every
(`FORMAT_VERSION`, `SCHEMA_HASH`) pair kiln has shipped, one line each,
`<version> <16 hex digits>`, oldest first. The rules:

- The last line is this build's pair.
- A change of `SCHEMA_HASH` bumps `FORMAT_VERSION` by one and appends the
  new pair. A line is never edited or removed, versions increase, and no
  hash has two versions.

`src/pch_snapshot.rs` `tests::format_record` fails when they do not
hold, and says what to do: for a schema hash that changed under the same
version, `the snapshot schema hash changed (<old> -> <new>) without a
format version bump: set FORMAT_VERSION in
crates/kiln_driver/src/pch_snapshot.rs to <N+1> and append `<N+1>
<new>` to crates/kiln_driver/pch-snapshot-format.txt`; for a bumped
version without its line, `FORMAT_VERSION is <N> but ... ends at <M>:
append `<N> <hash>` to it` (`tests::format_record_messages` checks each
message). A consumer refuses a snapshot of another version or schema
hash with a warning and replays ("Consumer"), so the version tells a
reader why, and the record tells a reviewer that a schema change was
meant.

`SHAPE_HASH` is not in the record: `SCHEMA_HASH` folds every shape a
cycle closes on, so a change of a `SHAPE_HASH` that a snapshot can see
already changes `SCHEMA_HASH`, and a per-type list would add nothing a
reader checks.

The record starts with the schema kiln 1.2.0 ships; versions 1 and 2
before it are described above. A PCH from another kiln build ends in a
correct compile either way (`tests/pch_snapshot.rs` `cross_version`):

| the PCH | result |
|---|---|
| another snapshot format version | warning `-Wpch-snapshot-unused`, replay |
| another schema hash | warning `-Wpch-snapshot-unused`, replay |
| no snapshot section (kiln 1.1.0's, or `-fno-kiln-pch-snapshot`) | replay, `-v` note only |
| another `kiln` line in the record | `error: PCH file built from a different branch (<theirs>) than the compiler (<ours>)`; with `-fno-validate-pch` the record is not checked, and the snapshot is judged by its own identity |
| another kiln build in the snapshot's identity (same schema) | warning `-Wpch-snapshot-unused` (`kiln build differs`), replay |
| `KILNPCH 1` | `error: PCH file '<p>' uses an older format that is no longer supported` |

## Identity

The producer writes, and the consumer computes, the same components and
compares them in this order. The first that differs refuses the
snapshot with the warning (`-Wpch-snapshot-unused`), and `-v` names it:
`note: precompiled header 'p.pch': snapshot not used: target differs:
'aarch64-unknown-linux-gnu' (producer) vs 'x86_64-pc-linux-gnu'
(here)`, or for a component of several lines, `-D/-U list differs at
line 2: '-DA=1' (producer) vs '-DA=2' (here)`. Each component is
derived from the parsed configuration, so two spellings that reach the
same state agree (`tests/pch_snapshot.rs` `identity_components`;
`src/pch_snapshot.rs` `tests` for the language and `-verify` cases):

| component | what it is | covers |
|---|---|---|
| (schema hash) | checked first, before the identity is decoded | the layout of every root |
| kiln build | `kiln <version> <commit>` | the build that wrote it |
| working directory | the current directory; not compared when the prefix does not depend on it (below) | relative paths: the header, `-I`, `__FILE__` |
| language | the resolved standard (`LangStd`: C or C++, the standard, GNU or not; a C header's snapshot is refused by a C++ consumer and by another C standard) | `-std=`, `-ansi`, `-x`, the driver mode's default |
| target | the triple and the resolved target features | `--target`, `-march`, `-mcpu`, `-m<feature>`, `-target-feature` |
| include search path | the resolved search list (each directory with its characteristic: `-v`'s text does not tell `-I` from `-isystem`) and the `--embed-dir` list; equal text is the fast path, other text is compared by the prefix's lookups (below) | `-I`, `-iquote`, `-isystem`, `-idirafter`, `-cxx-isystem`, `--sysroot`, `-isysroot`, `--gcc-toolchain`, `--gcc-install-dir`, the GCC installation found, `-stdlib`, `--llvm-dir`, `-resource-dir`, `-nostdinc*` |
| -D/-U list | `-D`/`-U` in order | the command-line macros |
| predefined macros | the predefines buffer without the `-D`/`-U`s and the `-include`s | everything that makes a predefined macro (`-O`, `-f[no-]exceptions`, `-fsanitize=`, `-fclang-compat=`, ...) |
| feature queries | the `__has_feature`/`__has_extension` flags in effect and the compat level | the raw `-f` options those read |
| warning policy | `DiagOptions::policy_fingerprint`: the policy's flags, every id mapped away from its default, and the last sign of each named group | `-W...`, `-w`, `-pedantic[-errors]`, `-R...` |
| prefix maps | `-fmacro-prefix-map`/`-ffile-prefix-map`, in order | `__FILE__` and source-location spellings |
| source path style | `-fkiln-source-path-style=` | the same |
| reproducible time | the fixed epoch, if any | `-fkiln-reproducible-time`, `KILN_REPRODUCIBLE_TIME`, `SOURCE_DATE_EPOCH` |
| options | every other parsed option (`Args`, pretty-printed, one field per line) | anything not listed here, including options added later |

In the options component, `-O0` is no `-O` (WP-1404: every reader of the
level treats them alike, so a PCH built without `-O` serves a `-O0 -c`
consumer); other levels stay as spelled.

The warning policy is the one component where option order can differ
and still match: two `-W` lists give the same text exactly when the
policies they build cannot be told apart by any diagnostic (the policy
reads the order of its `-W` options only to find the last sign of a
group). So `-Wshadow -Wunused-variable` and `-Wunused-variable -Wshadow`
match; `-Wall -Wno-unused-variable` and `-Wno-unused-variable -Wall` do
not. Every other list stays in order.

Left out of the identity (`pch_snapshot::excluded`), because they cannot
change the state at the end of the prefix:

| options | why |
|---|---|
| the inputs, `-o`, `-c`, `-S`, `-emit-llvm`, `-fsyntax-only`, `-E`, `--precompile`, `-fmodule-output`, the program name | the main file's compile and where it goes; the prefix is the same |
| `-include` | the PCH's own are matched by the record; the consumer's later ones are entered after the restore |
| `-include-pch` and the PCH options (`-f[no-]kiln-pch-*`, `-emit-pch`, `-fno-validate-pch`) | the mechanism itself |
| `-M*`, `-MD`, `-MMD`, `-MF`, `-MT`, `-MQ`, `-MP`, `-MG`, `-MJ` | dependency output is written from the record's file list and the main file's |
| `-g*` and the debug prefix maps | read only by code generation |
| `-v`, `--version`, `--help`, `-print-*`, `-###` | reports |
| `-fdump-*`, `-fkiln-dump-cfg`, `-fkiln-cfg-stats`, `-fkiln-memory-report`, `-ftime-trace*`, `-fkiln-bmi-image-census*` | reports about the main file's compile |
| `-verify`, `-verify=`, `-verify-ignore-unexpected` | they check the diagnostics the job emits; when a `-verify` consumer restores is in "Consumer" |
| how diagnostics are shown: colour, `-fdiagnostics-*`, `-ferror-limit=`, `-fmacro-backtrace-limit=`, `-ftabstop=`, carets, columns | a prefix with errors gets no snapshot, so the limit cannot have stopped it, and the rest only formats text |
| crash reporting (`-f[no-]crash-diagnostics*`, `-fkiln-crash-for-testing=`) | only after an internal error |
| `-fkiln-backend=`, `--hod=`, `-disable-llvm-passes`, `-Wa,`, `-Xassembler`, linker options, `-Qunused-arguments`, `-fmodules-prune-*` | after the front end, or cache housekeeping |

Command-line errors are left out too: a compile with any does not run.

## Lookup-tolerant search path

Build systems give each translation unit its own `-I` list: the PCH is
built once, and each consumer adds its own directory (some put one
first). clang does not require equal search lists for a PCH, and neither
does the snapshot (WP-1403).

**The lookup log.** The producer records every header lookup the prefix
performed (spec/pp.md "Lookup log"): for each `#include`,
`#include_next`, `#import`, `__has_include`, `__has_include_next`,
`#embed` and `__has_embed` evaluated (nothing is looked up in a skipped
group), its key (the spelling, `"..."` or `<...>`, the includer's
directory where the form tries it, and for the `_next` forms the
directory the search continues after) and its result (not found; found
relative to the includer; or found in search directory `D` with `D`'s
characteristic, user, system or `extern "C"` system). A lookup of an
absolute name does not use the search list and is not recorded (its
file is in the record's file list). Equal keys are recorded once.

**The comparison.** The consumer compares the `include search path`
component by text first: equal (and the working directory equal) is the
fast path, and nothing more is checked. Otherwise, after the file check,
it repeats each recorded lookup in order with its own search list (and
its own `--embed-dir` list, working directory and file system) and
compares the results: all equal restores; the first that differs refuses
the snapshot with the warning, and `-v` names it: `include search path:
lookup of <vector> now finds '/w/shadow/vector' (was
'/usr/include/c++/14/vector')`, or `(was not found)`, `now finds
nothing`, `now finds '/w/inc/h' as a system header (was a user header)`.
An `#include_next` that continues after a directory the consumer's list
does not have is a difference (`... continues after '/d', which is not
in the search list`). Tests: `tests/pch_snapshot_search.rs`.

**A damaged log** (WP-1396). The log is read after the block checksum has
accepted the section. Its counts are bounded by the bytes left
(spec/snap.md "Threat model"), and `LookupLog::validate` checks every
directory index (each result's, includer's, `after` and search-list
entry) against its directory table, and the `<...>` start against the
list. A log that fails is a section that cannot be read: the snapshot is
refused with a `-v` note (`its lookup log names directory 7 of 3
(result)`), not with the warning, as for any other decoding error. A
log that passes is repeated as written; its names and directories are
only joined into paths and asked about, so an empty, `..`, absolute, NUL
or very long name is a file that is not there, or another file
(`tests/pch_snapshot_fuzz.rs` `snapshot_corruption_fuzz`).

**Soundness.** (1) The prefix's tokens depend on the search path only
through its lookups' results (the predefines and the feature queries,
which also read options, are components of their own); a result fixes
the file, the name it is known by, whether it is a system header, and
where an `#include_next` in it continues. (2) A result is a function of
its key, the search list and the file system; a key is a function of
earlier results (the includer's name, the directory it was found in).
(3) So, by induction over the producer's lookups in order, a consumer
whose search list gives every recorded key the recorded result performs
the same lookups with the same results, reads the same files (checked
by the file check) and reaches the same prefix.

**Repeating a lookup** asks the file system whether a path is a regular
file, without reading it, and asks only where the producer did not look:
a directory the producer's lookup searched without finding the name (its
list from where the lookup started to its result, and the includer's
directory of a `"..."` lookup found elsewhere) is not asked again, nor is
the file a lookup found when the record's file list has it (the file
check has just seen it). This takes the file system to be the
producer's except for the files the record lists, which is what the
fast path takes too; it holds for a relative directory only in the
producer's working directory, so from another one those are asked. A
directory that misses four times is read once as a listing (spec/pp.md
"Lookup caching"). The cost is in docs/status/pch-snapshot-gates.md
("Search path").

*Corner cases* (`tests/pch_snapshot_search.rs`):

| consumer | result |
|---|---|
| adds a directory that holds none of the prefix's headers (first, last, between) | restores (`unrelated_directories_restore`) |
| reorders directories so that every lookup finds the same file | restores (`unrelated_directories_restore`) |
| adds a directory that shadows a prefix header | refused: `now finds '<dir>/<h>'` (`shadowing_refuses_and_the_replay_re_resolves`) |
| adds a directory that makes a failing `__has_include` succeed | refused: `now finds ... (was not found)` (`has_include_that_now_succeeds_refuses`) |
| gives a prefix directory as `-isystem` instead of `-I` (or back) | refused: the characteristic is part of the result (`the_consumers_list_is_in_force`) |
| `-iquote` | as `-I`, for `"..."` lookups only |
| `-idirafter` | a system directory searched last |
| `-isysroot`, `--sysroot`, `--gcc-toolchain`, `-resource-dir`, `-nostdinc*` | through the search list they build |
| `-stdlib=` | in `options` too (it also selects sema's library builtins), so it still refuses there |
| `CPATH`, `C_INCLUDE_PATH`, `CPLUS_INCLUDE_PATH` | kiln does not read them (no effect on either side) |
| a relative `-I` with the same text in another working directory | the working directory refuses, unless the prefix does not depend on it (below) (`working_directory`) |
| `#include_next` in the prefix | continues after the same directory in the consumer's list (`include_next_in_the_prefix`) |
| `#embed` in the prefix, `--embed-dir` | repeated with the consumer's `--embed-dir` list (`embed_lookups`) |

**Working directory.** The prefix does not depend on the working
directory when every file the record lists and every result of the
lookup log (the includer's directory of a relative hit, the search
directory of a hit, an `#embed` resource's path) is an absolute path:
file names, `__FILE__`, the restored source manager's lazy reads and the
diagnostics' paths are then the same from any directory. The consumer
then leaves the `working directory` component out, and repeats the
lookups when its directory differs. A prefix that names any file
relatively (a header given as `-include h.h` or built as `h.h`) keeps
the component (`tests/pch_snapshot_search.rs` `working_directory`).

## Producer

A producer (spec/driver.md "Precompiled headers") checks the header
exactly as `-fsyntax-only` does. Any error means no PCH is written and
the exit status is 1; a header with warnings gets a PCH and a snapshot
(`tests/pch_snapshot.rs` `werror_prefix_has_no_snapshot`). It then
writes the record and, unless `-fkiln-pch-snapshot=load` or
`-fno-kiln-pch-snapshot` is in force ("Options"), the snapshot section.

**The snapshot pass.** The section is the state after a pass over an
empty main file whose `-include`s are the producer's own followed by the
header (`pch_snapshot::produce`). That pass is the *single pass*
(WP-1393, spec/snap.md "The producer's single pass") when it may stand
in for the header pass, else it runs after the ordinary PCH job (*two
passes*).

- The single pass is not tried (`pch_snapshot::single_pass_blocker`) for
  a C header, with `KILN_PCH_SNAPSHOT_PASSES=2`, `-v`, `-verify`, a time
  trace, `-fkiln-memory-report`, `-fdump-ast`, `-fdump-record-layouts`,
  `-fdump-vtable-layouts`, `-fkiln-dump-cfg`, `-fkiln-cfg-stats`,
  `-fkiln-bmi-image-census`, `-fconstexpr-report`, constexpr profiling,
  or `-MG`.
- A single pass stands in for the header pass only when it wrote no
  diagnostics, the end of its translation unit gave no error, no
  instantiation report and no CFG dump, and nothing a main file alone
  gets could differ (`pch_snapshot::stands_in`: no `#include_next`,
  `#pragma system_header`, module or import line or `#pragma ...
  diagnostic` in the header; no main-file-only unused-declaration
  warning; no `__INCLUDE_LEVEL__` or `__BASE_FILE__`; the header found
  under its own name as a user header). Otherwise the header pass runs,
  and its result is the job's.

Either shape writes the same section and the same PCH, and reports the
same diagnostics and dependency list (`tests/pch_snapshot_producer.rs`
`the_single_pass_stands_in_for_a_quiet_header`,
`the_header_pass_runs_when_a_main_file_could_differ`;
`tests/pch_snapshot_search.rs` `single_pass_modes_and_reasons`). With
`KILN_SNAP_TIMING=1` the producer prints `snapshot-timing: producer single
pass: stands in for the header pass`, or `header pass needed (<why>)`.

**Refusals.** A header without errors whose prefix cannot be
snapshotted still gets its PCH, with no section. The reasons
(`syntax_cmd.rs` `SyntaxJob::snapshot_encode`, `kiln_parse::parse_prefix`):

- the target is not supported, or the preprocessor could not be built;
- the prefix has errors (`the prefix has N error(s)`);
- a module image or carried constants are being recorded;
- a part of the state is not at rest at the end of the header
  (`<Type>.<field>: <why>` from the quiescence checks: `Sema`, the
  parser and the preprocessor; for example a header that ends in a
  global module fragment's `module;` is `Parser.mods: not at rest`);
- the prefix's header lookups were not recorded;
- an encoding error, or a section block that cannot be built (`the
  section block: <why>`).

Each is reported once: `warning: precompiled header 'p.pch' cannot carry
a snapshot (<why>); its consumers process the header again
[-Wpch-snapshot-unused]` (`drv_pch_snapshot_not_written`), and `-v`
notes `snapshot not written: <why>`. The consumers of such a PCH replay
with a `-v` note only (`tests/pch_snapshot_producer.rs`
`a_prefix_without_a_snapshot_still_gets_a_pch`).

**`-Werror`.** The group `pch-snapshot-unused` is exempt from a blanket
`-Werror` (its ids are `WarningNoWerror`, as clang's
`DefaultWarnNoWerror`): with `-Werror` the warning stays a warning and the
PCH is written. `-Werror=pch-snapshot-unused` promotes it; only then
does a producer that cannot snapshot fail, with exit status 1 and no PCH
written (`tests/pch_snapshot_producer.rs`
`a_prefix_without_a_snapshot_still_gets_a_pch`).
`-Wno-pch-snapshot-unused` silences it.

## Consumer

A compile with `-include-pch p.pch` (or a PCH found by the `-include`
probe, or picked from `-include-pch <dir>`) takes these steps in order
(`crate::pch::consumer`):

1. **Read the record** as a stream ("File layout"). An unreadable,
   foreign, clang, corrupt, older or newer record is an error
   (spec/driver.md "Precompiled headers", the record's diagnostics; the
   `-E`/`-M` exception there). `tests/pch.rs`
   `stale_missing_and_foreign`; `tests/pch_snapshot.rs` `cross_version`.
2. **`-E` and `-M`** replay without validating: they preprocess the
   header the PCH was built from, then the `-include`s.
3. **Validate the record** (unless `-fno-validate-pch`): the `kiln` line
   ("built from a different branch"), the language options, the target,
   the macros, then every non-system file's size and content, with
   clang's diagnostics and order (spec/driver.md "Precompiled headers").
   Any failure is an error and the compile stops. `tests/pch.rs`
   `option_mismatches`, `stale_missing_and_foreign`.
4. **Consumers that always replay**, with a `-v` note
   (`pch::snapshot_refused`): a module unit (an input whose first
   declaration, after comments and directives, is `module` or `export
   module`; a replay puts the header before its module declaration,
   which is an error, while a restore would accept it as clang's PCH
   does), and a `-verify` consumer whose header has a file that may
   hold a directive of one of its prefixes (a replay lexes the header,
   and its directives count). `src/pch.rs` `module_units`,
   `verify_directives`; `tests/pch_snapshot.rs` `verify_consumers`,
   `chained_and_module_consumers`.
5. **Find the snapshot** (`pch_snapshot::find`): the section in the
   block; its header; the schema hash; the identity; the lookup log's
   validity; the file check; the lookups when the search list or the
   working directory differs. The first that fails refuses the snapshot.
6. **Restore** the roots and go on with the main file, then the
   consumer's own `-include`s that the producer did not have. A restore
   that gives up during the job replays (`snapshot not used (<why>); the
   header is replayed`).
7. Otherwise **replay**: the compile behaves like `<consumer args>
   -include <producer's -include>... -include <producer's main>
   -include <consumer's -include>...`. A consumer `-include` the producer
   already had is dropped (CMake's `-include-pch p.pch -include
   cmake_pch.hxx` would otherwise enter its unguarded `cmake_pch.hxx`
   twice). A relative path is resolved against the producer's working
   directory when the consumer's is different. `tests/pch.rs`
   `replay_order_and_cwd`, `cmake_producer_and_consumer`.

**The file check** (`pch_snapshot::file_changed`), every file the record
lists, system files included:

- `-fkiln-pch-validate=mtime` (the default): a file with the recorded
  size and modification time is unchanged and is not read. Another size
  is changed. The same size with another time is read and hashed: the
  same text is accepted (a touched file), other text is changed. A
  record without a time (0) is hashed. This is clang's check, with the
  content as the fallback rather than an error.
- `-fkiln-pch-validate=content`: every file is read and hashed.

`tests/pch_snapshot.rs` `file_check`. An edit that keeps both the size
and the modification time is not detected in `mtime` mode (as in
clang); `content` detects it. The restored source manager re-reads a
prefix file only when its text is first needed (a caret line, a
presumed location, `-g`), and checks it against the recorded length and
content hash then: a file that no longer matches ends the compilation
with `fatal error: file '<f>' has been modified since the precompiled
header '<p.pch>' was built: content changed` (`drv_pch_file_modified`)
after the consumer's own diagnostics, with exit status 1 and no output;
it never compiles against text it did not check. A `#line` directive
whose recorded physical line does not match the text is treated the
same (spec/source.md "Snapshots"). `tests/pch_snapshot.rs`
`prefix_file_changed_after_the_check`.

**Every reason to replay.** Each prints `note: precompiled header
'<pch>': snapshot not used: <why>` under `-v` (and `snapshot-timing:
note: not used: <why>` under `KILN_SNAP_TIMING=1`). The configuration
reasons also warn once, before the job: `warning: precompiled header
'p.pch' was built with a different configuration (<why>); the header is
processed again [-Wpch-snapshot-unused]` (`drv_pch_snapshot_unused`).

| reason | warns |
|---|---|
| `-fno-kiln-pch-snapshot`, `=write` (the consumer replays by request) | no (no note either) |
| `=check` (restores and replays; "Options") | no |
| `-E`, `-M` | no (no note) |
| a module unit; a `-verify` consumer with directives in the header (step 4) | no |
| the PCH has no snapshot section | no |
| the section block cannot be read (`<why> in its section block`) | no |
| the section is truncated, or has another magic | no |
| the section has another format version | **yes** |
| another schema hash (`written by another build (schema hash)`) | **yes** |
| the head or the lookup log cannot be decoded | no |
| an identity component differs (`<component> differs ...`) | **yes** |
| a file the record lists changed (`'<f>' changed`, `changed size`, `is gone`) | **yes** |
| a lookup finds something else (`include search path: ...`) | **yes** |
| the roots cannot be read from the file | no |
| the restore gives up during the job | no |

`tests/pch_snapshot_search.rs` `the_warning_is_controllable`,
`by_design_replays_do_not_warn`; `tests/pch_snapshot.rs`
`identity_components`, `section_header`, `cross_version`, `file_check`;
`tests/pch_snapshot_fuzz.rs` `snapshot_corruption_fuzz`.

As at the producer, the group is exempt from a blanket `-Werror`; only
`-Werror=pch-snapshot-unused` promotes the warning, and the compile then
stops as for a rejected PCH (exit status 1). `-Wno-pch-snapshot-unused`
silences it (`tests/pch_snapshot_search.rs`
`the_warning_is_controllable`).

**`-verify`.** A `-verify` consumer restores, and the verifier sees the
main file's (and the later `-include`s') comments and the consumer's
diagnostics. It replays when the header reported any warning or error
when it was built (a directive expecting one, e.g.
`expected-warning@p.h:1`, could not be satisfied by a restore) or in
the step-4 case. clang 19 instead never sees the header's diagnostics or
directives with `-include-pch` (`tests/pch_snapshot.rs`
`verify_consumers`).

**State carried across the PCH.** Identical to replay
(`tests/pch_snapshot.rs` `open_state_at_the_end_of_the_prefix`,
`c_header`; `tests/pch_decls.rs`): `#pragma push_macro` without its pop,
`#pragma clang diagnostic push` without its pop, `#pragma pack(push)`,
`#pragma GCC visibility push`, `#pragma once` in the header (the main
file's `#include` of it is skipped), `#pragma STDC FP_CONTRACT`, a
pending `#pragma weak` (the alias form too), a pending `#pragma
redefine_extname`; C headers (tentative and K&R definitions,
`_Static_assert`, `#pragma pack`, VLA typedefs in the main file). A
header that ends inside a `#pragma clang attribute push` or inside `#if`
is an error at the producer (no PCH). `#pragma STDC FENV_ACCESS ON` is
an error in any header.

**Dependency output.** The consumer's `-MD` list is the same as
replay's: the PCH's input files (as recorded, relative to the
producer's directory), then the main file and its headers, as clang
lists them with `-include-pch`. With `-include p.h` the main file comes
first, as clang's (`tests/pch_snapshot.rs` `depfile_order`;
`tests/pch.rs` `user_only_depfile`).

**Chained and module consumers.** A PCH built with another
`-include-pch` gets a snapshot of the whole prefix, and its consumer
restores. A consumer with `-fmodule-file=` restores when the producer
had the same options (they are in the identity)
(`tests/pch_snapshot.rs` `chained_and_module_consumers`,
`module_fragment_repeats_the_prefix`).

## Diagnostics from the prefix

The header's own diagnostics are shown when the PCH is built and not
again when it is used, as with clang's PCH. The restored engine counts
only the consumer's, so the summary line is clang's (`1 warning and 4
errors generated.`). `-v` says how many were not shown (`snapshot
restored; the N warning(s) and error(s) its header reported when it was
built are not shown again`). A replay still shows them: its first N
warnings and errors are the header's ("Deviations from clang").
`tests/pch_snapshot.rs` `header_diagnostics_are_not_shown_again`;
`tests/pch_decls.rs` `check_accepts_header_warnings`.

State that affects later diagnostics is carried: the `#pragma clang/GCC
diagnostic` mappings and an open `push`, and the deduplication set
(spec/diag.md §7.4: a diagnostic the prefix printed at a place is not
printed again for the same place and context). Warnings kiln gives once
per entity keep their tables in `Sema` (spec/sema.md "Snapshots"): as
with replay, a template the main file instantiates again warns again at
the same place with the new context, an undefined internal function used
in the prefix is reported once at the end of the unit, and a deprecated
entity is reported at each use (`tests/pch_snapshot.rs`
`diagnostic_state_is_carried`; `tests/pch_snapshot_producer.rs`
`a_prefix_leaves_end_of_unit_warnings_to_its_consumer`). `-Werror`,
`-Wfatal-errors`, `-ferror-limit=` and `-ftemplate-backtrace-limit=` stay
the consumer's.

## Options

- **`-f[no-]kiln-pch-snapshot[=write|load|check]`**, the last one wins
  (`tests/pch_snapshot.rs` `modes`, `check_mode`,
  `check_mode_prefix_warns`):

  | option | producer | consumer |
  |---|---|---|
  | (none; the default since WP-1404) | writes the section | restores it |
  | `-fkiln-pch-snapshot` | writes the section | restores it |
  | `=write` | writes the section | replays |
  | `=load` | writes none | restores it |
  | `=check` | writes the section | restores it **and** replays, compares, reports a difference |
  | `-fno-kiln-pch-snapshot` | writes none | replays |

  `=check` compiles the main file to `-fdump-ast=locs` twice in the same
  process, once restored and once replaying, and compares (1) whether
  the snapshot could be restored at all, (2) the diagnostics, and (3)
  the dump. For (2), the `N warnings generated.` lines are left out and
  replay's diagnostics must be the restored ones, preceded only by lines
  without an error (the header's own warnings). The first difference is
  the error `the snapshot of precompiled header '<pch>' differs from
  replaying the header: <what> differs at line N: '<snapshot>'
  (snapshot) vs '<replay>' (replay)` (`drv_pch_snapshot_mismatch`), and
  the compile fails. The job's own results (diagnostics, dump, IR, `-MD`
  list) are replay's. It is the debugging mode: three front-end runs.
- **`-fkiln-pch-validate=mtime|content`**: the file check ("Consumer");
  `mtime` by default.
- **`-fkiln-pch-image[=write|check|load]`** (WP-1397) is accepted and
  ignored: it selected the declaration-image PCH of kiln 1.1, which the
  snapshot replaced. The driver warns once per invocation, naming the
  last one written: `warning: '-fkiln-pch-image' is ignored: the
  precompiled header snapshot replaced it [-Woption-ignored]`
  (`drv_pch_image_ignored`). `-fno-kiln-pch-image` is accepted silently;
  the last of the two wins. Any other value is still `invalid value
  '<v>' in '-fkiln-pch-image=<v>'`. The PCH and the compile are those
  without the option (`tests/pch.rs` `retired_image_option_is_ignored`).
- The options are listed in `--help` ("PRECOMPILED HEADERS").

**Environment.**

- `KILN_SNAP_TIMING=1` (any value but empty or `0`) prints
  `snapshot-timing:` lines on standard error: the producer's pass shape
  and encode time, the consumer's find and restore split, the
  `snapshot-timing: note: <what>` form of every `-v` snapshot note, and
  the process phases. The harness reads them (spec/harness.md "Snapshot
  accounting"); `tests/pch_decls.rs` and `tests/pch_snapshot_producer.rs`
  use them to tell a restore from a replay.
- `KILN_PCH_SNAPSHOT_PASSES=2` makes the producer take two passes
  ("Producer"), for a comparison of the two shapes
  (`scripts/pch-snapshot-diff.sh`).

## Deviations from clang

docs/COMPATIBILITY.md "Precompiled headers" lists these for users.
Tests: `tests/pch.rs` `stale_missing_and_foreign` (the clang PCH and
the same-size edit; `tests/pch_snapshot.rs` `file_check` the touch), `include_probes_for_pch` (the probe,
compared with clang++-19), `preprocess_unreadable_pch` (`-E`);
`tests/pch_snapshot.rs` `cross_version`, `header_diagnostics_are_not_shown_again`,
`verify_consumers`, `chained_and_module_consumers`,
`module_fragment_repeats_the_prefix`.

- A kiln PCH is not a clang PCH, and neither reads the other's: a clang
  PCH given to kiln is `fatal error: file 'p.pch' is a precompiled header
  built by clang; kiln reads only its own precompiled headers`.
- A touch that leaves the content unchanged is accepted, and a same-size
  edit is `content changed`; clang compares modification times (`mtime
  changed (was N, now M)`) or misses an edit made within the same
  second.
- A refused snapshot replays, with `-Wpch-snapshot-unused` where the
  configuration differs; clang rejects a PCH whose configuration differs
  (an error) and has no fallback. The record's own checks are clang's
  errors.
- A replay consumer reports the header's warnings again; a restore, like
  clang, does not.
- A `-verify` consumer replays when the header reported diagnostics or
  may hold directives; clang never sees them.
- The `.gch` probe accepts kiln's and clang's PCH magic, not an object
  file with a `__clangast` section, which clang also accepts.
  `-Wp,-include,<h>` is probed, since kiln splices `-Wp,` into the
  driver's arguments; clang does not probe it.
- `-fpch-preprocess` is an unknown argument; `-fpch-codegen` and
  `-fpch-debuginfo` are accepted and emit nothing (a PCH is not an
  object).
- clang's `-E` reads the header name from one of its own PCHs and
  preprocesses it; kiln reports a clang PCH and goes on without it.
- A module unit that kiln builds in-process for `-fmodule-file=M=a.cppm`
  begins with the importer's predefines, which carry its `-include` and
  `-include-pch` lines. So the PCH also applies to the module unit, and a
  declaration it adds before `module;` is an error there. A two-step
  build with the same flags behaves the same. clang never builds a module
  unit from an importer's command line.
