# spec: kiln_diag

Status: normative for WP-002 (crate `crates/kiln_diag`). "MUST" and
"SHOULD" carry their RFC 2119 meanings. The reference behavior is
**clang 19** (`clang++-19`, the version in the container). Where this
document says "as clang", kiln's output for the same input, the same
options and the same locations MUST be byte-identical to clang's, except
for the numbered deviations in §11. Every test in `crates/kiln_diag/tests`
names the section it checks with a `// spec: diag.md §N` comment.

## 1. Scope

`kiln_diag` owns:

- the diagnostic table (ids, kinds, groups, message templates), §2-§3;
- diagnostic options and the warning policy derived from them, §4-§5;
- the `Diag` builder that components use to report, §6;
- the `DiagEngine`, which applies policy, counts, deduplicates and hands
  records to a sink, §7;
- the layout of a diagnostic into located entries (macro backtraces), §8;
- the text renderer (clang's format) and the JSON renderer, §9-§10.

It depends on `kiln_source` only (spans, files, expansion records) and on
no third-party crate. It contains no `unsafe` (`#![forbid(unsafe_code)]`).

## 2. Identifiers and templates

### 2.1 The id table

`DiagId` is a `#[repr(u16)]` enum generated from one table in `src/ids.rs`.
Each row is

```text
name = Kind [Group] (nargs) "template";
```

- `name` is `snake_case` with a component prefix (`lex_`, `pp_`, `parse_`,
  `sema_`, `oven_`, `cg_`, `drv_`, `diag_` for the engine's own, `note_`
  for shared notes). The name is the stable identifier: it appears verbatim
  in JSON output and test expectations, and MUST NOT be renamed once
  merged. `DiagId::ALL[id as usize] == id`; `DiagId::from_name` inverts
  `name`.
- `Kind` (`DiagKind`) fixes the class and default severity, mirroring
  clang's TableGen classes:

  | Kind              | clang              | class     | default  |
  |-------------------|--------------------|-----------|----------|
  | `Error`           | `Error`            | Error     | Error    |
  | `Fatal`           | `DefaultFatal`     | Error     | Fatal    |
  | `Warning`         | `Warning`          | Warning   | Warning  |
  | `WarningOff`      | `DefaultIgnore`    | Warning   | Ignored  |
  | `WarningError`    | `DefaultError`     | Warning   | Error    |
  | `WarningNoWerror` | `DefaultWarnNoWerror` | Warning | Warning (never promoted by `-Werror`) |
  | `Extension`       | `Extension`        | Extension | Ignored  |
  | `ExtWarn`         | `ExtWarn`          | Extension | Warning  |
  | `Remark`          | `Remark`           | Remark    | Ignored  |
  | `Note`            | `Note`             | Note      | (follows its parent) |

- `[Group]` is optional; hard errors (`Error`, `Fatal`) MUST NOT have one.
- Where clang has the same diagnostic, the template wording, kind and group
  MUST be clang's.

### 2.2 Templates and the static argument check

A template is text with placeholders:

- `{N}`: argument `N` (0-based decimal).
- `{N:alt0|alt1|...}`: a *select*: argument `N` read as a decimal index
  picks an alternative; a non-numeric or out-of-range value picks the last
  alternative. Alternatives may contain `{M}` but not nested selects.
- `{{` and `}}` are literal braces (outside alternatives).

`template::check(tmpl, nargs)` is a `const fn` that is true iff the
template is well-formed and uses every index in `0..nargs` and no other.
The table MUST assert it for every row in a `const` item, so a mismatch
between a template and its declared argument count fails the build.

### 2.3 Rendering

`template::render(tmpl, args)` substitutes arguments. Missing arguments
render as nothing; a malformed template renders its remaining text
verbatim. The builder (§6) debug-asserts that the number of `.arg()` calls
equals `nargs`.

## 3. Warning groups

`Group` is generated from a table in `src/groups.rs`:
`Variant = "clang-name" [Subgroup, ...];`. A group's members are the ids
naming it, the members of its subgroups (transitively), and, for
`pedantic` only, every `Extension`/`ExtWarn` id without a group (clang
places those in `-Wpedantic`). `display_group` is the id's group, or
`pedantic` for a groupless extension; it is what `[-W...]` and JSON
`"group"` show.

Group names and nesting MUST match clang's for every group kiln declares
(e.g. `all` ⊃ `most` ⊃ `unknown-pragmas`; `pragmas` ⊃ `unknown-pragmas`).

`CLANG_GROUP_NAMES` lists every group name clang 19 accepts
(`clang++-19 --print-diagnostic-options`). A `-W`/`-R` option naming one of
these that kiln has no members for MUST be accepted silently, so build
flags tuned for clang (and the libc++ test suite) do not produce
`-Wunknown-warning-option`.

## 4. Options

`DiagOptions::parse_arg(arg) -> bool` consumes diagnostic options in
command-line order and returns false for anything else.
`DiagOptions::from_args` folds it over a list. Defaults are the clang
*driver's*:

| Option | Effect | Default |
|---|---|---|
| `-w` | ignore all warnings (not positional) | off |
| `-W<x>` | appended to `warnings` (not `-Wl,` `-Wa,` `-Wp,`) | |
| `-WX` | alias of `-Werror` | |
| `-R<x>` | appended to `remarks` (not `-Rpass*`) | |
| `-pedantic`, `--pedantic` | extensions at least warnings | off |
| `-pedantic-errors`, `--pedantic-errors` | extensions at least errors | off |
| `-ferror-limit=N` | §7.3; 0 is unlimited | 19 |
| `-fmacro-backtrace-limit=N` | §8.2; 0 is unlimited | 6 |
| `-ftabstop=N` | 1..=100, else warning `drv_ignoring_tabstop` and 8 | 8 |
| `-f[no-]color-diagnostics`, `-f[no-]diagnostics-color`, `-fdiagnostics-color=always\|never\|auto` | §9.2 | off |
| `-f[no-]caret-diagnostics` | source snippets | on |
| `-f[no-]diagnostics-show-line-numbers` | `  12 \| ` gutter | on |
| `-f[no-]diagnostics-show-option` | ` [-Wgroup]` suffix | on |
| `-f[no-]show-column` | `:col` in locations | on |
| `-f[no-]show-source-location` | `file:line:col: ` prefix | on |
| `-f[no-]diagnostics-fixit-info` | fix-it insertion lines | on |
| `-fdiagnostics-format=clang\|json` | default sink format | clang |

Bad values are recorded in `pending` as `OptionDiag`s and emitted by the
engine first (§7.1), with clang's wording:
`invalid integral value 'abc' in '-ferror-limit abc'`,
`invalid value 'xml' in '-fdiagnostics-format xml'`,
`ignoring invalid -ftabstop value '0', using default value 8`.

## 5. Policy

### 5.1 Building the mapping

The policy is a transcription of clang's `ProcessWarningOptions`. Each id
has a mapping `{severity, user, no_werror, no_fatal}` initialised from its
kind (`no_werror` set for `WarningNoWerror`). Then all `-W` options are
applied in order, followed by all `-R` options. For a `-W` option
(after stripping `no-`, which makes it negative; `format=0` means
`no-format`):

- `system-headers`: accepted, no effect (kiln has no system headers yet).
- `everything`: positive sets the global `enable_all`; negative maps every
  warning/extension to Ignored.
- `error`: sets/clears the global warnings-as-errors flag.
- `error=G`: positive maps G's members to Error; negative (`no-error=G`)
  demotes members currently Error/Fatal to Warning and sets `no_werror`.
  `error-implicit-function-declaration` is clang's legacy alias. Any other
  `error<x>` is an unknown specifier.
- `fatal-errors`, `fatal-errors=G` / `fatal-errors-G`: the same for Fatal
  (global errors-as-fatal flag; `no_fatal`).
- otherwise `G`: positive maps members to Warning, negative to Ignored.

Mapping an id (clang's `setSeverity`) sets `user`; mapping to Warning an id
already mapped Error/Fatal leaves it Error/Fatal (so `-Werror=G -WG` stays
an error, while `-Werror=G -Wno-G` ignores it). `-R` options map remark
groups to Remark/Ignored; `-Reverything` maps every remark.

### 5.2 Severity of an emitted id

`severity(id)` (clang's `getDiagnosticSeverity`), in order:

1. Notes: never emitted on their own (they follow their parent).
2. `r` = the mapped severity.
3. If `enable_all`, `r` is Ignored, the id is not user-mapped and not a
   remark: `r` = Warning.
4. If the id is an extension and not user-mapped: `r = max(r, ext)`,
   where `ext` is Error under `-pedantic-errors`, Warning under
   `-pedantic`, else Ignored. (`-Wno-error=G` does not set `user`, so
   `-pedantic-errors -Wno-error=extra-tokens` still errors.)
5. If `r` is Ignored, stop.
6. Under `-w`: Ignored if `r` is Warning, or if `r` ≥ Error and the id is
   not an error by default. Hard errors and `DefaultError` warnings
   survive `-w`.
7. Warning becomes Error under `-Werror` unless `no_werror`.
8. Error becomes Fatal under `-Wfatal-errors` unless `no_fatal`.

The behaviour matrix in `tests/policy.rs` was taken from clang 19 and MUST
keep passing.

### 5.3 Diagnostics about the options

After applying, every option is examined again (clang's second, reporting
pass), so a later `-Wno-unknown-warning-option` silences an earlier unknown
option. These are produced, in option order:

- `unknown warning option '-W<x>'` / `'-Wno-<x>'` / `'-Werror=<x>'` /
  `'-Wfatal-errors=<x>'`, and `unknown remark option '-R<x>'`, when the
  name is neither a kiln group nor in `CLANG_GROUP_NAMES`; with
  `; did you mean '<prefix><best>'?` when one kiln group of that flavor
  with members has the uniquely smallest edit distance, at most the
  name's length (a tie suggests nothing);
- `unknown -Werror warning specifier: '-W<x>'` (and `-Wfatal-errors`).

## 6. Building diagnostics

```rust
Diag::new(DiagId::pp_macro_redefined, name_span)
    .arg("FOO")                                   // template arguments, in order
    .range(name_span)                             // underline (unlabeled)
    .span(other, "label")                         // labeled range (label: JSON only)
    .note(DiagId::note_previous_definition, prev) // argument-less note id or free text
    .with_note(Diag::new(DiagId::note_macro_here, s).arg("FOO")) // full note
    .fixit(FixIt::replace(span, "text"))          // also FixIt::insert / FixIt::remove
    .allow_duplicate()                            // opt out of §7.4
    .emit(&engine);
```

- The caret goes at `span.lo`. The primary span is **not** underlined;
  clang's `^~~~` is `.range(span)` on the primary span.
- All spans are `kiln_source` half-open spans and may be macro locations;
  §8 maps them to the file.
- `Diag` is `#[must_use]`. Notes MUST only be attached, never emitted
  directly; free-text messages exist only for notes.

## 7. The engine

### 7.1 Interface

```rust
DiagEngine::new(&sm, &opts, sink) // sink: impl DiagSink + Send
DiagEngine::with_stderr(&sm, &opts) // TextSink or JsonSink per opts.format
engine.emit(diag) / diag.emit(&engine)
engine.severity(id) / is_enabled(id)
engine.error_count() / warning_count() / has_fatal() / summary() / flush()
```

`new` first emits `opts.pending`, then §5.3's reports, as locationless
diagnostics. `DiagSink::emit(&mut self, sm, &Diagnostic)` receives
records: id, level, rendered message, primary span, labels, fix-its,
option suffix, and attached notes (macro-expansion notes are *not* in the
record; renderers derive them, §8). Provided sinks: `TextSink<W>`,
`JsonSink<W>`, `MemorySink` (clones share storage; `take`,
`diagnostics`) and `SharedBuffer` (a cloneable `io::Write`).

`emit` processes a diagnostic in this order:

1. If a fatal error was emitted, drop it (§7.2).
2. Compute the severity (§5.2); drop if Ignored.
3. Deduplicate (§7.4).
4. Apply the error limit (§7.3).
5. Count: Error and Fatal count as errors, Warning as warnings.
6. Hand the record to the sink.

Steps 3-6 happen under one lock, so concurrent emitters see one total
order and the sink is called serially.

The option suffix (clang's `printDiagnosticOptions`) is `-Werror` if the
level is Error, the id is a warning or extension and not an error by
default, then `-W<display group>` (`-R` for remarks), comma-separated. A
fatal error never shows `-Werror` (clang prints
`fatal error: hi [-W#warnings]` under `-Werror -Wfatal-errors`).

### 7.2 Fatal errors

Once any diagnostic is emitted at level Fatal, every later diagnostic is
dropped, and `has_fatal()` is true. Components SHOULD poll `has_fatal()`
to stop early.

### 7.3 Error limit

With limit `N ≠ 0`, an Error emitted when `N` errors have been counted is
replaced by the locationless fatal `too many errors emitted, stopping now
[-ferror-limit=]` (id `diag_too_many_errors`), which counts as an error;
everything after it is dropped (§7.2). Warnings never count toward the
limit. With the default 19, a TU with 30 errors prints 19 errors, the
fatal, and `20 errors generated.`

### 7.4 Deduplication

A diagnostic with a valid primary span is dropped if one with the same
`(id, span)` was already emitted, unless it is `allow_duplicate`.
Diagnostics with invalid spans are never deduplicated. Ignored
diagnostics are not recorded.

### 7.5 Summary

`summary()` is `None` with no warnings and no errors, else clang's
`N warning(s)`, `M error(s)`, or `N warning(s) and M error(s)` followed by
` generated.` (singular for 1). The driver prints it after the sink's
output.

### 7.6 Thread safety

`DiagEngine` is `Send + Sync` and every method takes `&self`, so one
engine MAY be shared by parallel workers (scoped threads). Counters are
atomics; the sink and dedup set are under a mutex. Exactly one
too-many-errors fatal is produced however emitters interleave.

## 8. Layout (macro backtraces)

A record is laid out into a sequence of entries: the diagnostic, its
macro-expansion notes, then each attached note followed by its own
expansion notes. This is a transcription of clang 19's
`DiagnosticRenderer` over `kiln_source`'s expansion records.

### 8.1 Locations

The reported location of a macro location is clang's `getFileLoc`: the
spelling location if it is inside a macro argument, else the expansion
location, repeatedly. Presumed file names and lines (`#line`) are used for
the header; columns are raw byte columns on the original line.

### 8.2 Macro backtrace

For a macro location, one `note: expanded from macro 'NAME'` per
expansion level is emitted, innermost first, each with its own snippet
(clang's `emitMacroExpansions`, `retrieveMacroLocation`). The name is
the text of the invocation's name token; a level whose spelling lies in
the buffer named `SCRATCH_BUFFER_NAME` (`<scratch space>`, used by the
preprocessor for pasted and stringized tokens) prints `note: expanded
from here` instead. With limit `L ≠ 0` and more than `L` levels, the first
`L/2 + L%2` and the last `L/2` are shown with a
`note: (skipping K expansions in backtrace; use -fmacro-backtrace-limit=0 to see all)`
between them.

### 8.3 Range mapping

Each range is mapped to the level of the entry it is shown with (clang's
`mapDiagnosticRanges`): both ends are walked up the expansion chain until
they are in the same expansion as the caret; ranges that cannot be mapped
are dropped from that entry.

### 8.4 Macro arguments

If the caret and every range are in the same macro argument, levels that
expand only the argument are skipped (clang's
`checkRangesForMacroArgExpansion`). Kiln checks only the begin and end of
each range (deviation D7).

### 8.5 Expansion model expected from the preprocessor

The layout is only as good as the expansion records. The preprocessor
MUST record, for clang-equivalent output:

- a `Macro { name }` entry for each invocation, whose expansion span is
  the whole invocation (`NAME(args)`), with an exclusive `hi`;
- a `MacroArg` entry for each substituted argument token run, whose
  expansion span is the parameter token in the body;
- for a pre-expanded argument, the `MacroArg` spelling is the macro
  location produced by expanding the argument;
- for a nested invocation, the expansion span lies inside the enclosing
  entry (`[base + a, base + b)`).

## 9. Text output

### 9.1 Header

`<file>:<line>:<col>: <level>: <message> [<option>]`

- The location prefix is omitted for invalid locations and under
  `-fno-show-source-location`; `:<col>` is omitted under
  `-fno-show-column`.
- Levels print as `note`, `remark`, `warning`, `error`, `fatal error`.
- The option suffix (§7.1) is omitted when empty or under
  `-fno-diagnostics-show-option`; notes have none.
- No word wrapping (D3).

### 9.2 Colors

With colors on, as clang: the location is bold (`ESC[1m`); the level is
`ESC[0;1;35m` (warning), `ESC[0;1;31m` (error, fatal), `ESC[0;1;36m`
(note), `ESC[0;1;34m` (remark); the message is bold except for notes and is
followed by `ESC[0m`; the caret line is `ESC[0;1;32m`, the fix-it line
`ESC[0;32m`, and non-printable bytes are reverse video (`ESC[7m`).
`-fdiagnostics-color=auto` means off (kiln does not probe the terminal).

### 9.3 Snippets

Under `-fcaret-diagnostics`, an entry with a file location is followed by
clang's `emitSnippetAndCaret`:

- the lines from the first to the last line any range touches on the
  caret's file (at most 16 lines, clang's limit), each printed as
  `<gutter> | <line>` then the caret/underline line `<pad> | <marks>`;
- the gutter is right-aligned in `max(4, digits(last line + 16))`
  columns under `-fdiagnostics-show-line-numbers`, else absent;
- `^` at the caret column, `~` under ranges (clang's
  `prepareAndFilterRanges` / `highlightRange`, including the multi-line
  rules that trim leading and trailing whitespace);
- trailing whitespace on the marker line is trimmed; lines longer than
  4096 bytes are not shown;
- CRLF and lone CR terminators render as LF.

### 9.4 Printable text

Lines are shown with tabs expanded to the tab stop. A byte that is not
part of valid printable UTF-8 is shown as `<XX>` (hex, uppercase); a
non-printable code point as `<U+XXXX>`. Kiln shows the *original* bytes
of invalid UTF-8 (restored from the source manager's load report), as
clang does. Column arithmetic for carets and ranges uses display columns
(clang's `SourceColumnMap`).

### 9.5 Fix-its

Under `-fdiagnostics-fixit-info`, insertion fix-its on a shown line produce
a line under the caret line with the inserted text at its column (clang's
`buildFixItInsertionLine`), only when every fix-it is in the caret's file
and on one line. Replacement and removal ranges are underlined like ranges.

### 9.6 Include stack

Before a diagnostic in a file other than the last one reported, clang
prints `In file included from <file>:<line>:` lines. Kiln has the
hook (`include_stack` in `src/text.rs`) but `kiln_source` has no include
records yet, so nothing is printed (D8).

## 10. JSON output

### 10.1 Structure

`-fdiagnostics-format=json` writes one JSON object per diagnostic on one
line, in emission order:

```json
{"id":"pp_macro_redefined","severity":"warning","file":"a.c","line":2,"col":9,
 "message":"'A' macro redefined","group":"macro-redefined","option":"-Wmacro-redefined",
 "notes":[{...same fields without "notes"...}],
 "spans":[{"file":..,"line":..,"col":..,"end_line":..,"end_col":..,"label":".."}],
 "fixits":[{"file":..,"line":..,"col":..,"end_line":..,"end_col":..,"text":".."}]}
```

- `id` is the `DiagId` name, `null` for free-text notes. Macro-expansion
  notes appear in `notes` with ids `diag_expanded_from_macro`,
  `diag_expanded_from_here`, `diag_backtrace_skipped`.
- `severity` is `note`, `remark`, `warning`, `error` or `fatal`.
- `file`/`line`/`col` are as the text header (presumed file and line, raw
  byte column) and all `null` for locationless diagnostics.
- `group` is the display group, `option` the bracketed suffix, or `null`.
- `spans` lists only labeled ranges (`.span()`, including `.range()`'s
  empty label), mapped as in §8; ends are exclusive.
- `fixits` lists fix-its whose ends are both file locations (as clang's
  parseable fix-its); others are omitted.

### 10.2 String escaping

`escape_json` escapes `"` and `\`, uses `\n \r \t \b \f`, writes other
U+0000-U+001F and U+007F as `\u00xx` (lowercase hex), and copies
everything else, non-ASCII included. Output lines never contain raw
control characters.

## 11. Deviations from clang 19

| # | Deviation | Why |
|---|---|---|
| D1 | No syntax highlighting of keywords/literals in colored snippets. | Needs a lexer in `kiln_diag`; cosmetic. |
| D2 | Adjacent fix-its are not merged (`mergeFixits`). | No caller needs it yet. |
| D3 | No word wrap / `-fmessage-length`. | The driver default is no wrapping. |
| D4 | `-fdiagnostics-color=auto` means off. | No terminal probing. |
| D5 | Unicode printability and width are approximate (ASCII exact; combining/wide ranges coarse). | clang uses LLVM's full tables; no third-party deps here. |
| D6 | Unknown-option suggestions consider kiln's groups only, so e.g. `-Wbogus` suggests `-Wmost` where clang's larger table ties and suggests nothing, and `-Rbogus` suggests nothing where clang suggests `-Rpass`. | Kiln lacks clang's group membership data. |
| D7 | Macro-argument range checks look at range ends only. | Kiln spans are half-open; clang walks token by token. |
| D8 | No include stack (`In file included from`). | `kiln_source` has no include records yet. |
| D9 | Selects are index-based (`{0:a\|b}`); there is no `%plural`. | Callers pass the index. |
| D10 | `-WX` is accepted as `-Werror`. | MSVC-style alias some build systems pass. |
| D11 | JSON is kiln's own schema (with `option`, `fixits`, severity `fatal`), not clang's SARIF. | Built for the test harness. |
| D12 | `pedantic` contains exactly the groupless extensions. | clang's `-Wpedantic` also has named subgroups kiln does not have yet. |
| D13 | Policy is per TU; `#pragma clang diagnostic` is not supported. | Per-location state is future work (§12). |

## 12. Integration notes and open questions

- Lexer/preprocessor MUST pass `.range()` for underlines and MUST build
  expansion records as §8.5 describes; pasted/stringized tokens go in a
  buffer named `<scratch space>`.
- `#pragma clang diagnostic push/pop/ignored` needs per-location policy
  (clang's `DiagStatePoints`); the policy is immutable today.
- The include stack (§9.6) needs `include_loc`/`add_inclusion` in
  `kiln_source` (WP-014).
- `kiln_source` could expose a line-start query; `kiln_diag` derives it
  from `line_text` pointer arithmetic.
