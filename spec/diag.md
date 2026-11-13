# spec: kiln_diag

Status: normative for WP-002 (crate `crates/kiln_diag`). "MUST" and
"SHOULD" carry their RFC 2119 meanings. The reference behavior is
**clang 19** (`clang++-19`, the version in the container). Where this
document says "as clang", kiln's output for the same input, the same
options and the same locations MUST be byte-identical to clang's, except
for the numbered deviations in §12. Every test in `crates/kiln_diag/tests`
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

`DiagId` is a `#[repr(u16)]` enum generated from the table in
`src/ids/*.rs` (laid out per §2.4). Each row is

```text
name = Kind [Group] (nargs) "template";
```

- `name` is `snake_case` with a component prefix (`lex_`, `pp_`, `parse_`,
  `sema_`, `oven_`, `cg_`, `drv_`, `diag_` for the engine's own, `note_`
  for shared notes). The name is the stable identifier: it appears verbatim
  in JSON output and test expectations, and MUST NOT be renamed once
  merged. `DiagId::ALL[id as usize] == id`; `DiagId::from_name` inverts
  `name`. `ALL` follows the table: area files in file-name order, rows by
  name within a file, then any tail rows (§2.4). Discriminants are
  internal indexes only (never serialized), so adding an id may renumber
  others.
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

### 2.4 Table layout (WP-236)

The id and group tables are split into area files so parallel branches
add rows to different files, or at least different places in a file,
instead of all appending to one table end. `build.rs` concatenates
`src/ids/*.rs` and `src/groups/*.rs` (file-name order, comments and blank
lines dropped) into the `diag_id_table!` / `diag_group_table!` macros,
which `src/ids.rs` / `src/groups.rs` expand together with their *tail*
block, a `diag_id_table! { }` / `diag_group_table! { }` at the end of the
file. Area files hold one row per line, sorted by name; a comment directly
above a row belongs to it; each file starts with a generated header.

- Id `P_rest` lives in `ids/P.rs`, `P` being the text before its first
  `_`. A large area is split by name range into `ids/P_<lo>_<hi>.rs`
  (`sema_a_b`, `sema_c_d`, …): the row goes to the file with the greatest
  `lo` not above the first letter of `rest` (the first range also takes
  anything below its `lo`). Ranges of an area MUST be contiguous, and an
  area is either one file or ranges, never both. A new prefix gets a new
  `P.rs`.
- Groups with subgroups, and `Pedantic`, live in `groups/umbrellas.rs`,
  which names children by variant. A leaf group lives in
  `groups/<prefix>.rs` for the prefix of one of the ids naming it (it stays
  in its current file while that holds; otherwise the most common prefix,
  ties to the smaller name), and in `groups/misc.rs` if no id names it.
- The tail blocks and `ids/_pending.rs` / `groups/_pending.rs` are landing
  zones: rows there compile, but MUST be moved before merging.

`cargo xtask diag-split` moves tail and `_pending` rows into their area
files and re-sorts and re-places every row; it is idempotent.
`cargo xtask diag-split --check` (run by `cargo xtask ci`) fails if a tail
or `_pending` file holds rows or a row or file is out of place.
`cargo xtask diag-split --replay [<base> <head>]` re-applies the table
edits of `<base>..<head>` to the working tree by row name (3-way: added,
changed and removed rows; for a group changed on both sides, subgroup
additions and removals are merged), reporting rows changed on both sides
as conflicts. With no revisions it runs inside a merge: base is
`merge-base HEAD MERGE_HEAD`, head is `HEAD`, and a conflicted or
pre-split `ids.rs`/`groups.rs`, and any conflicted area file, is first
reset to `MERGE_HEAD`'s version. Code outside the tables that the branch
changed in `ids.rs`/`groups.rs` is not replayed; the tool warns.

Bringing a branch up to date with main when the tables conflict
(including a branch from before the split):

```text
git merge origin/main             # conflicts in kiln_diag ids/groups files
cargo xtask diag-split --replay   # takes main's tables, replays your rows
git add crates/kiln_diag && git commit
```

To add ids by hand, append rows to the tail block (or `_pending.rs`) and
run `cargo xtask diag-split`, or write them straight into the area file.

### 2.5 Wording by clang compatibility level (WP-377)

An id's template is clang 22's wording. Where clang 19 worded the same
diagnostic differently, the table also has a row `<name>_19` with clang
19's template and the same kind, group and argument count
(`tests/ids.rs` checks the mirror). `DiagId::for_compat(level)` maps an id
to the one that reports it at `-fclang-compat=<level>`: below 22, `<name>`
becomes `<name>_19`; otherwise the id is itself. The engine applies it to
every diagnostic and note it emits (`DiagOptions::clang_compat`, set by
the driver from `-fclang-compat=`; default 22, as the driver), and to its dedup key, so
code emits `<name>` and never picks a `_19` row itself. A message
embedded in another diagnostic (sema's "substitution failure ...: <msg>")
is rendered with `Diag::message_at(engine.clang_compat())`. Differences
that are not a template (an argument printed differently, a note clang 22
adds or drops) are chosen by the emitting crate from its own level
(`Sema::clang_compat`, `Oven::set_clang_compat`).

## 3. Warning groups

`Group` is generated from the table in `src/groups/*.rs` (§2.4):
`Variant = "clang-name" [Subgroup, ...];`. A group's members are the ids
naming it, the members of its subgroups (transitively), and, for
`pedantic` only, every `Extension`/`ExtWarn` id without a group (clang
places those in `-Wpedantic`). `display_group` is the id's group, or
`pedantic` for a groupless extension; it is what `[-W...]` and JSON
`"group"` show.

Group names and nesting MUST match clang's for every group kiln declares
(e.g. `all` ⊃ `most` ⊃ `unknown-pragmas`; `pragmas` ⊃ `unknown-pragmas`).

Where clang 22 nests a group differently, the extra edges are keyed by the
level (WP-394): `Group::compat_subgroups(level)` lists them (at 22,
`c99-designator` ⊃ `initializer-overrides`, `reorder-init-list`), and
`Group::members_at(level)` / `contains_at(id, level)` follow them;
`members()` / `contains(id)` are level 19. The policy (§5.1) and the
pragma state (§5.5) resolve `-W`/`-Wno-` and `#pragma clang diagnostic`
group names at `DiagOptions::clang_compat`
(`dcl.init.aggr/c99-designator-group.cpp`).

`CLANG_GROUP_NAMES` lists every group name clang 19 accepts
(`clang++-19 --print-diagnostic-options`). A `-W`/`-R` option naming one of
these that kiln has no members for MUST be accepted silently, so build
flags tuned for clang (and the libc++ test suite) do not produce
`-Wunknown-warning-option`.

### 3.1 Umbrella groups (WP-230)

An umbrella lists every subgroup kiln implements, in the order clang 19's
`DiagnosticGroups.td` lists them; a clang subgroup kiln has no diagnostic
for is left out rather than declared empty, so that the group table only
holds groups that have members. Membership is checked against clang by
probe (`clang++-19 -fsyntax-only -Wno-everything -W<umbrella>`), and the
probes are recorded in the tests named below. `extra` holds
`deprecated-copy`, `initializer-overrides` and `unused-but-set-parameter`
(WP-865).

| Umbrella | Kiln's subgroups (clang order) | Tests |
|---|---|---|
| `all` | `most`, `switch`, `switch-bool`, `vla-cxx-extension` | `intro.compliance/wall-*.cpp`, `dcl.array/vla-gnu-mode-wall.cpp` |
| `most` | `comment`, `format`, `multichar`, `range-loop-construct`, `reorder`, `return-type`, `sizeof-array-argument`, `sizeof-array-decay`, `tautological-compare`, `trigraphs`, `uninitialized`, `unknown-pragmas`, `unused`, `overloaded-virtual` | `intro.compliance/wmost-members.cpp`, `intro.compliance/wmost-tautological-compare.cpp` |
| `bool-conversion` | `pointer-bool-conversion`, `undefined-bool-conversion` | `intro.compliance/wbool-conversion-members.cpp` |
| `unused-value` | `unused-comparison`, `unused-result`, `unevaluated-expression` | `intro.compliance/wunused-value-unevaluated.cpp` |
| `extra` | `deprecated-copy`, `initializer-overrides`, `unused-but-set-parameter` | `depr.impldec/deprecated-copy.cpp`, `dcl.fct/unused-but-set-parameter-bare-w.cpp` |
| `format` | `format-extra-args`, `nonnull`, `format-security`, `format-invalid-specifier`, `format-insufficient-args` | `tests/c/expr/static-array-argument.c` (WP-1312) |
| `shadow-all` | `shadow` (⊃ `shadow-field-in-constructor-modified`), `shadow-field-in-constructor` (⊃ the same), `shadow-uncaptured-local`, `shadow-field` | `basic.scope.block/shadow*.cpp`, `class.base.init/shadow-*.cpp` |
| `gnu` | `gnu-case-range`, `gnu-conditional-omitted-operand`, `gnu-designator`, `vla-extension`, `gnu-folding-constant`, `gnu-imaginary-constant`, `gnu-include-next`, `gnu-line-marker`, `gnu-pointer-arith`, `gnu-string-literal-operator-template`, `gnu-zero-line-directive`, `gnu-zero-variadic-macro-arguments` | `ext.gnu.extension/wgnu-*.cpp` |

`conversion` holds `bool-conversion`, `literal-conversion`,
`constant-conversion`, `enum-conversion`, `implicit-float-conversion` and
`null-conversion` (WP-408 added `bool-conversion`, whose warnings are on
by default). `implicit-float-conversion` ⊃ `implicit-int-float-conversion`
⊃ `implicit-const-int-float-conversion` (on by default; WP-865,
spec/landed/sema/WP-865.md). `c++11-narrowing` ⊃
`c++11-narrowing-const-reference`. `deprecated` holds `deprecated-copy`
and `deprecated-copy-with-dtor`; `deprecated-copy` ⊃
`deprecated-copy-with-user-provided-copy`, `deprecated-copy-dtor` ⊃
`deprecated-copy-with-dtor` ⊃ `deprecated-copy-with-user-provided-dtor`.
`unused` holds `unused-lambda-capture`. The `shadow` family is in no
umbrella but `shadow-all`; clang's `shadow-ivar` (Objective-C) is
accepted as a name only.

`unevaluated-expression` is reached from `most` through `unused` ⊃
`unused-value`, as in clang, so `-Wmost -Wno-unused-value` turns it off.
`vla-cxx-extension` being in `all` matters in a GNU dialect only, where
its diagnostic is an off-by-default Extension.

`deprecated` holds `deprecated-array-compare`, `deprecated-declarations`,
`deprecated-missing-comma-variadic-parameter`,
`deprecated-redundant-constexpr-static-def` and `deprecated-this-capture`
(clang order). The WP-356 groups follow clang 22: `array-compare-cxx26`
(its one id is `WarningError`; `-Wno-array-compare-cxx26` silences the
C++26 array comparison, as in clang),
`array-compare`, `deprecated-array-compare`, `tautological-compare` (in
`most`), `invalid-static-assert-message` (`WarningOff` in kiln, where
clang makes it `DefaultError`: [dcl.pre]/12 makes the message it
evaluates an unevaluated operand; spec/sema.md §4.6, WP-570) and
`c++26-extensions` (the `ExtWarn`s for C++26 features used earlier:
placeholder variables, user-generated `static_assert` messages).

Clang subgroups kiln does not have yet (add them, in place, when their
diagnostics land):

- `all`: `parentheses`, `misleading-indentation`, `packed-non-pod`.
- `most`: `array-bounds`, `bool-operation`, `char-subscript`,
  `delete-non-virtual-dtor`, `extern-c-compat`, `for-loop-analysis`,
  `frame-address`, `implicit`, `infinite-recursion`,
  `int-in-bool-context`, `mismatched-tags`, `missing-braces`, `move`,
  `private-extern`, `self-assign`, `self-move`, `string-plus-int`,
  `user-defined-warnings`, `volatile-register-var`; under `unused`:
  `unused-argument`, `unused-function`, `unused-label`,
  `unused-local-typedef`, `unused-private-field`;
  under `uninitialized`: `sometimes-uninitialized`.
- `gnu` (clang order): `gnu-alignof-expression`, `gnu-anonymous-struct`,
  `gnu-auto-type`, `gnu-binary-literal`, `gnu-complex-integer`,
  `gnu-compound-literal-initializer`,
  `gnu-empty-struct`, `gnu-flexible-array-initializer`,
  `gnu-flexible-array-union-member`, `gnu-label-as-value`,
  `gnu-null-pointer-arithmetic`, `gnu-offsetof-extensions`,
  `redeclared-class-member`, `gnu-redeclared-enum`,
  `gnu-statement-expression`, `gnu-union-cast`,
  `gnu-variable-sized-type-not-at-end`, `zero-length-array`. The probe
  shows `gnu-static-float-init` is not in clang 19's `gnu`.

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
| `-Wframe-larger-than=N` (and its bare alias `-Wframe-larger-than`) | clang's backend stack-frame limit, a driver option: N must be an integer, else error `drv_invalid_argument_to_option` (`invalid argument 'abc' to -Wframe-larger-than=`); kiln emits no frame-size warning, so a valid N is accepted and has no effect (WP-811, BoringSSL). `-Wframe-larger-than` with `-Wno-`/`-Werror=` is the warning group | |
| `-Wlarger-than=<x>`, `-Wlarger-than-<x>` | GCC options clang ignores, accepted with any value (WP-811) | |
| `-W` | alias of `-Wextra` (clang's old spelling; WP-682, `dcl.fct/unused-but-set-parameter-bare-w.cpp`) | |
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

- `system-headers`: positive clears, negative sets the global
  suppress-system-warnings flag (default set; §5.4). The last one wins.
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

### 5.4 System headers

clang's `SuppressSystemWarnings` rule (the end of
`getDiagnosticSeverity`). A diagnostic is dropped, after §5.2 (so a
warning made an error by `-Werror`, `-Werror=G` or `-pedantic-errors` is
dropped too), when all of these hold:

- the flag is set (no `-Wsystem-headers`);
- the id does not show in system headers. `DiagId::shows_in_system_header`
  is true for hard errors (`Error`, `Fatal`) and for the warnings clang
  marks `ShowInSystemHeader`: `pp_warning_directive` (`#warning`),
  `oven_enum_constexpr_conversion` (checked against clang 19) and
  `sema_diagnose_if_succeeded_warning` (a `diagnose_if` warning, which
  libc++ issues from its headers; WP-486). Every other
  warning, `DefaultError` warnings included, is suppressed;
- the primary span is valid and `SourceManager::is_in_system_header(lo)`:
  the expansion location of the caret is in a system header as the
  preprocessor marked it (spec/source.md "File kinds": `-isystem` and the
  default system directories, `#pragma GCC system_header` from the pragma
  on, line-marker flag 3 from the next line on).

A warning in a user file produced by a macro defined in a system header
is kept (its expansion location is in the user file), except that clang's
second rule (WP-1227) also drops it, under the same flag, when:

- the id does not show in system macros: `DiagId::shows_in_system_macro`
  is false only for the ids clang 19 marks `SuppressInSystemMacro`, of
  which kiln has `sema_decl_shadow` (`warn_decl_shadow`; clang's other two,
  `warn_old_style_cast` and `ext_cxx_designated_init`, have no kiln id
  yet). Every other id, `sema_decl_shadow_uncaptured_local` included,
  shows;
- `SourceManager::is_in_system_macro(lo)` (clang `isInSystemMacro`): the
  caret is a macro location whose spelling location is in a system header,
  so a macro-body token of a system macro, not a macro argument written in
  a user file. A caret spelled in `<scratch space>` (a `##` paste) first
  steps outward with `immediate_macro_caller_loc` until it is spelled
  elsewhere, then the rule applies to that location.

sema's `-Wshadow` check asks `is_enabled_at(sema_decl_shadow, name loc)`
before looking anything up (clang `shouldWarnIfShadowedDecl`), so a
declaration spelled in a system macro is not checked for
`-Wshadow-uncaptured-local` either (the nested `__ret` locals of the
NEON intrinsic macros in `arm_neon.h`). Notes go with their parent.
`DiagEngine::suppress_system_warnings()` exposes the flag (kiln_pp skips
macro-redefinition checks in system headers when it is set, as clang
does) and `is_enabled_at(id, loc)` combines §5.2 with this rule.

### 5.5 Pragma state (WP-183)

`#pragma clang diagnostic` and `#pragma GCC diagnostic` change the
mapping by location (clang `DiagStateMap`, `pragma.rs`). The
preprocessor calls `DiagEngine::pragma_push(at)`, `pragma_pop(at)` (false
when nothing was pushed: kiln_pp's "could not pop") and
`pragma_set(at, remark, group, severity)` for
`ignored`/`warning`/`error`/`fatal` (false for an unknown group;
`everything` is every warning, extension and remark of the flavor, as
`setSeverityForAll`), with `at` the expansion location of the `pragma` /
`_Pragma` token.

- A pragma makes a new state current from `at` on: the current state's
  overrides plus, for each group member, a user mapping with the
  pragma's severity that keeps the old `no_werror` (`-Wno-error=`) and
  drops `no_fatal`. Unlike a command-line `-Wfoo`, a `warning` pragma
  overrides an error mapping. `pop` makes the state saved by `push`
  current again. Overrides go through §5.2 like command-line mappings
  (`Policy::severity_with`), so `-Werror`, `-w` and the extension rule
  still apply.
- The state at a location is the last transition point at or before its
  expansion location in the same buffer range, else the state at the
  range's include location, recursively (the command-line state at the
  top). When the preprocessor leaves an included file it calls
  `pragma_file_exit(resume)`, which adds a point at the includer's resume
  location if the current state differs there: a pragma in a header
  without `push`/`pop` reaches the rest of the includer, as in clang.
  An invalid location uses the current state.
- `emit`, `would_emit`, `is_enabled_at` and `severity_at(id, loc)` use
  the state at the diagnostic's location, so a diagnostic emitted late
  (an instantiation at the end of the TU) sees the pragmas around its
  location. `severity(id)` stays the command-line severity;
  `is_enabled(id)` is also true for an id some pragma enables. Sema's
  error checks (trailing notes, SFINAE) use `severity_at`.
- Cost: none until the first pragma (an atomic flag); then a read lock
  and a binary search over the transition points per severity query.

## 6. Building diagnostics

```rust
Diag::new(DiagId::pp_macro_redefined, name_span)
    .arg("FOO")                                   // template arguments, in order
    .range(name_span)                             // underline (unlabeled)
    .span(other, "label")                         // labeled range (label: JSON only)
    .note(DiagId::note_previous_definition, prev) // argument-less note id or free text
    .with_note(Diag::new(DiagId::note_macro_here, s).arg("FOO")) // full note
    .fixit(FixIt::replace(span, "text"))          // also FixIt::insert / FixIt::remove
    .context(key)                                 // dedup context, §7.4
    .allow_duplicate()                            // opt out of §7.4
    .emit(&engine);
```

- The caret goes at `span.lo`. The primary span is **not** underlined;
  clang's `^~~~` is `.range(span)` on the primary span.
- All spans are `kiln_source` half-open spans and may be macro locations;
  §8 maps them to the file.
- `Diag` is `#[must_use]`. Notes MUST only be attached, never emitted
  directly; free-text messages exist only for notes.
- `.error_note(text, span)` attaches a free-text note that prints and
  counts as an `error` (clang emits some `err_` ids as constant-evaluation
  notes, for example the -1 `__builtin_shufflevector` index). It follows
  its parent like any note.

### 6.2 Program-provided text (WP-1259)

Text that comes from the program rather than from kiln is escaped before
it reaches a message, by one function, `kiln_diag::escape_diag_text`
(clang's `pushEscapedString`, which clang applies to every string
argument): from the original bytes, ASCII whitespace (tab, newline,
vertical tab, form feed, carriage return) and printable or format (Cf)
characters print as they are; any other code point (a control character,
U+2028/U+2029, a private-use or noncharacter code point) as `<U+XXXX>`;
each byte that is not part of valid UTF-8 as `<XX>` (uppercase hex).
Printability follows §9.4 (an unassigned code point, such as U+0378,
escapes too). A newline is not escaped, as in clang: the
message continues on the next line. The callers:

- a failed `static_assert`'s message, a string literal or a `size()` /
  `data()` object (`Sema::static_assert_message_text`,
  `Sema::string_msg`; not truncated);
- the `what()` of an exception that leaves a constant evaluation, a
  `std::meta::exception` or one leaving a consteval block included
  (`Oven::exc_what`; capped at `WHAT_CAP` bytes before escaping,
  spec/oven.md §Exceptions);
- the messages of `deprecated` / `unavailable`, `nodiscard` (call and
  construction), `= delete("…")`, `enable_if` and `diagnose_if`
  (`Sema::string_msg`);
- a `data_member_spec` or `__kiln_define_member` name that is not an
  identifier, quoted in the `std::meta::exception`'s message (and so in
  its `what()`).

Kiln has no compile-time printing builtin; one would join this list.

### 6.1 Template type diffing (`%diff`)

Templates take pre-formatted strings, so a clang `%diff{...$...$...}`
pair is formatted by the producer: `Sema::ty_diff(from, to)` (kiln_sema
`type_diff.rs`) returns both quoted strings, and the template carries the
quotes-free `{0}`/`{1}` slots. It reproduces clang 19's `TemplateDiff` with
its defaults (elided, not tree form; `-fno-elide-type` and
`-fdiagnostics-show-template-tree` are not supported):

- Diffing applies only when both types (after their own sugar, aliases
  climbed as clang's `hasSameTemplate`) are specializations of the same
  class template. References and pointers at the top level are not
  diffed. Otherwise each side prints as a normal type argument (with its
  `aka`); a diffed side never has an `aka`.
- The template name prints unqualified (`tr<...>` for `N::tr`). Arguments
  are compared position by position over both written lists, packs
  flattened. An equal argument prints `[...]`, a run of N equal ones
  `[N * ...]`, and a fully equal list `pr<...>` (the types then differ
  in qualifiers only).
- A differing type argument recurses when both are specializations of one
  template and prints in full otherwise. A missing argument is the
  specialization's default prefixed `(default) `, or `(no argument)`.
  Qualifiers print common ones first, then the side's own.
- Two integral values of different types print `(T) V`; template template
  arguments print `template X`. Two type leaves that print the same are
  retried without elaboration, then canonically.

Users: `no known conversion` notes, `no viable conversion`, reference
binding errors, deleted/ambiguous conversions, `typedef redefinition`,
redeclaration with a different type, initializer-list `auto`
conflicts, overriding return types, assignment and conditional operand
mismatches, and `could not match ... against ...` deduction notes (which
report the innermost mismatching pair, canonical below the top level, and
the two template names for a template mismatch).

## 7. The engine

### 7.1 Interface

```rust
DiagEngine::new(&sm, &opts, sink) // sink: impl DiagSink + Send
DiagEngine::with_stderr(&sm, &opts) // TextSink or JsonSink per opts.format
engine.emit(diag) / diag.emit(&engine)
engine.severity(id) / is_enabled(id) / is_enabled_at(id, loc) / suppress_system_warnings()
engine.would_emit(&diag) // §7.4
engine.group_flag(group) // Option<bool>: the last of -W<group> / -Wno-<group>
engine.error_count() / warning_count() / has_fatal() / summary() / flush()
```

`group_flag` serves the few groups clang reads as language options
rather than severities (`-Winvalid-constexpr` sets
`CheckConstexprFunctionBodies`; spec/sema.md §5.28): only those two
spellings count, not `-Weverything`, `-Werror=<group>` or a pragma.

`new` first emits `opts.pending`, then §5.3's reports, as locationless
diagnostics. `DiagSink::emit(&mut self, sm, &Diagnostic)` receives
records: id, level, rendered message, primary span, labels, fix-its,
option suffix, and attached notes (macro-expansion notes are *not* in the
record; renderers derive them, §8). Provided sinks: `TextSink<W>`,
`JsonSink<W>`, `MemorySink` (clones share storage; `take`,
`diagnostics`) and `SharedBuffer` (a cloneable `io::Write`).

`emit` processes a diagnostic in this order:

1. If a fatal error was emitted, drop it (§7.2).
2. Compute the severity (§5.2); drop if Ignored, or if the system-header
   rule drops it (§5.4).
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
fatal, and `20 errors generated.`. With 19 errors there is no fatal.
Probed against clang 22 and clang 19, which agree (WP-406):
- 19, 20 and 21 errors at the default limit.
- `-ferror-limit=` 0 to 5, 19 and 20.
- Errors with notes, and errors from instantiation at end of TU.
- `#error`, a missing `#include`, and `-Werror` and pragma-promoted
  warnings.
Test `intro.compliance/error-limit-default.cpp`.

### 7.4 Deduplication

The deduplication key is `(id, span, context)`. `context` is a `u64`
set with `Diag::context(key)` (default 0, "no context"); sema sets it to
a hash of the template instantiation stack (every frame's kind, entity,
arguments and point of instantiation; never 0 for a non-empty stack,
spec/sema-templates.md §10.2). A diagnostic with a valid primary span is
dropped, unless it is `allow_duplicate`, if

- one with the same `(id, span, context)` was already emitted, or
- its context is not 0 and one with the same `(id, span)` and context 0
  was already emitted.

So an error in a template prints once per instantiation that hits it,
each with its own context notes, even when the messages are identical
(clang does not deduplicate at all); one instantiation reaching the same
location twice prints once; and a diagnostic already reported in the
template definition is not repeated by an instantiation that re-checks
the same non-dependent construct (clang reuses non-dependent nodes
instead of rebuilding them, so it never re-diagnoses them; sema marks
the exceptions `allow_duplicate`, e.g. the unused-result warnings of an
expression statement, which clang does rebuild). The reverse
does not hold: a context-free diagnostic after an instantiation's still
prints. A diagnostic marked `per_instantiation` skips only the second
rule: it is for checks clang re-runs on the node it rebuilds in every
instantiation even when the construct is non-dependent, so it prints in
the definition and once more per instantiation (`-Wreturn-stack-address`,
WP-231).

Diagnostics with invalid spans are never deduplicated. Ignored
diagnostics are not recorded.

`engine.would_emit(&diag)` answers, without recording anything, whether
`emit` would hand `diag` to the sink now: false after a fatal error, when
Ignored, when the system-header rule drops it, or when it is a duplicate
(the error limit is not considered). Sema uses it to attach the
once-per-depth instantiation-context notes only to a diagnostic that
will print, so a dropped duplicate does not consume them.

#### 7.4.1 Duplicates clang prints (WP-238)

clang has no deduplication, so wherever it runs one check twice on one
construct it prints the same diagnostic (and notes) twice at one
location. kiln follows clang in two ways:

- Where the repetition is observable behaviour clang users rely on,
  each use of the construct reports again, sema reproduces it with
  `allow_duplicate`; the corpus then lists the group once per print,
  as clang does. Known: an access error inside an alias template
  reported at the alias's `using` once per use (sema-templates.md §4.10,
  class/access.rs `AccessCheck::repeat`,
  `tests/conformance/class.access/type-names-alias-template.cpp`);
  the unused-result warnings of an expression statement clang rebuilds
  (above); the other `allow_duplicate` callers in kiln_sema each mirror
  a check clang re-runs.
- Where clang repeats only because its implementation runs one check
  twice, kiln prints once (the `(id, span, context)` key). Tests encode
  the second print with the `dup(clang)` suffix of the primary `diag:`
  directive (spec/harness.md "Directives"): the group is expected twice
  from clang and once from kiln, so strict tests stay exact under both.
  Known: "implicit instantiation of undefined template" for a
  declarator's type named through a qualifier into an undefined
  specialization (`U<int>::In x;`, at namespace and class scope; clang
  completes the qualifier for the nested-name-specifier and again for the
  declaration,
  `tests/conformance/temp.inst/undefined-template-qualifier-dup.cpp`).
  The repeat is per declaration, not per declarator: `X<int>::t f, &g;`
  and `static X<int>::t a, *b;` are two prints from clang and one from
  kiln, and a block-scope `X<int>::t a, &b;` (scope annotated by the
  statement disambiguation) is one from both (WP-330,
  `tests/conformance/temp.inst/undefined-template-qualifier-declarators.cpp`).
  New cases are added to this list with their test.

`-verify` has no such suffix: clang `expected-*` counts there are
clang's (D14).

### 7.5 Summary

`summary()` is `None` with no warnings and no errors, else clang's
`N warning(s)`, `M error(s)`, or `N warning(s) and M error(s)` followed by
` generated.` (singular for 1). The driver prints it after the sink's
output.

`error_count()` counts every error, including warnings promoted by
`-Werror`; it decides the exit status. Two narrower counters serve the
analyses that stop after an error:
- `uncompilable_error_count()` (clang `hasUncompilableErrorOccurred`):
  errors whose id is an error by default, and fatal errors. Sema's flow
  warnings at the end of a function body stop once it is non-zero.
- `unrecoverable_error_count()` (clang `hasUnrecoverableErrorOccurred`):
  errors whose id is of class error (not a warning or extension made an
  error by default or by `-Werror`), and fatal errors. A scope that saw
  one does not report its unused variables.

So a warning promoted by `-Werror` makes the run fail but does not
silence the later warnings.

### 7.6 Held diagnostics (WP-399)

`hold()` starts holding back every diagnostic but a fatal one; holds
nest, and the outermost `release()` emits the held ones not dropped, in
order, through the normal path (error limit, dedup, system headers then
apply). A held diagnostic counts as emitted while held (`error_count()`
and the other counters include it, so Sema's "an error occurred" checks
see it); `drop_held(i)` discards held diagnostic `i` and un-counts it.
`held_len()` is the index the next held diagnostic gets and
`held_errors_from(i)` lists the held errors from index `i` with their
ids. The user is Sema's clang 19 speculative typo correction
(spec/sema.md "Clang 19 correction to the qualified function"), which
drops the errors a failed speculation caused.

### 7.7 Thread safety

`DiagEngine` is `Send + Sync` and every method takes `&self`, so one
engine MAY be shared by parallel workers (scoped threads). Counters are
atomics; the sink and dedup set are under a mutex. Exactly one
too-many-errors fatal is produced however emitters interleave.

### 7.8 Snapshots (WP-1379)

`DiagEngine` is a configured root (spec/snap.md "Configured roots";
prototype, not called by the compiler yet; classification table at
`src/engine/snap.rs`). `Diag` and its parts, `Diagnostic`, `Level`,
`Severity` and the pragma state derive `Snap`; `DiagId` is written as
its `u16` index, and its schema descriptor hashes the id table's names.

- **State**: the `#pragma clang diagnostic` transition points, states and
  `push` stack (validated on restore: indices in range, points sorted),
  and the deduplication set (a diagnostic the prefix printed is a
  duplicate in the consumer).
- **Consumer-supplied**: the source manager, the policy built from its
  `-W` options, `-ferror-limit=`, `-fclang-compat=` and the sink.
- **Counts are not carried.** The error, warning, uncompilable and
  unrecoverable counts are per-compilation output: the restored engine
  keeps the consumer's own (its options diagnostics), so the summary
  line and the exit status count what the consumer printed, as clang's
  do with a PCH. Sema reads `uncompilable_error_count() > 0` and
  `unrecoverable_error_count()` to skip analyses after an error and
  `error_count()` deltas around speculative work; all agree with the
  producer only if the producer had no error, so a prefix with an error
  must not be snapshotted (the consumer-side decision, WP-1383).
- **Quiescence** (`snap_ready`): no `hold()` open, nothing held, no
  fatal error; `snap_put` refuses otherwise and names the field.
- **Restore** (`snap_restore`) needs a fresh engine (no pragma, nothing
  deduplicated) and must happen before the preprocessor that borrows the
  engine is built.

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

clang's `checkRangesForMacroArgExpansion` skips the levels that expand
only a macro argument when the caret and every range are in that
argument. It compares the ranges after `mapDiagnosticRanges`, which
yields spelling (file) locations, so in practice the skip applies only to
a diagnostic with no valid range whose caret is a macro-argument
location (e.g. `use of undeclared identifier` on a macro argument). Kiln
implements that observable rule (WP-203): a diagnostic that carries a
range (an expression range, a fix-it removal) keeps its `expanded from
macro` note for the argument's macro. Sema therefore attaches expression
ranges to the diagnostics clang gives ranges (unused results, static
assertion failures and their `expression evaluates to` note, invalid
operands, no matching function); a diagnostic kiln emits without a range
where clang has one loses the note.
Under clang 22 (`rangesInsideSameMacroArgExpansion`, unmapped ranges)
a fix-it counts with the range `mergeFixits` leaves it (WP-408,
`layout.rs` `merged_fixit_ranges`): an end in a macro argument moves to
where the argument is spelled and one at the start of a macro expansion
to its invocation (`edit::Commit::canInsert`), so an insertion before a
macro argument is a file range and keeps the notes; an end left in a
macro fails the commit and drops every fix-it range.

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

The character classes are LLVM 22's (`llvm/lib/Support/Unicode.cpp`),
exactly: kiln_diag's `unicode_tables.rs` holds sorted range tables that
`scripts/gen-unicode-tables.py` generates from UCD 15.1.0, the version
LLVM 22 generates its tables from (WP-1268; the generated file is checked
in and `@generated`; re-run the script, then `cargo fmt --all`):

- printable (`isPrintable`): general categories L, M, N, P, S and Zs,
  plus U+00AD SOFT HYPHEN (special-cased in code, as LLVM does); 711
  ranges. Unassigned code points are not printable;
- formatting (`isFormatting`, §6.2): general category Cf; 21 ranges;
- width 0 (`charWidth`): general categories Mn and Me; 343 ranges;
- width 2: East_Asian_Width F or W, plus U+1F300..U+1F5FF,
  U+1F900..U+1F9FF and U+4DC0..U+4DFF (LLVM's table carries the Yijing
  hexagrams although 15.1.0 gives them N); 100 ranges.

Lookup is a binary search over `(lo, hi)` pairs (about 9 KB of read-only
data). When the generator last ran, each table equalled LLVM 22's range
for range. clang 19's tables give the same answers for every code point
the corpus exercises (`dcl.pre/static-assert-message-unassigned-code-point.cpp`
passes under both).

### 9.5 Fix-its

Under `-fdiagnostics-fixit-info`, insertion fix-its on a shown line produce
a line under the caret line with the inserted text at its column (clang's
`buildFixItInsertionLine`), only when every fix-it is in the caret's file
and on one line. Replacement and removal ranges are underlined like ranges.

### 9.6 Include stack

clang's `emitIncludeStack`. Before a diagnostic whose include location
(`SourceManager::include_loc`, spec/source.md "Inclusions") differs from
the last one seen, the renderer prints `In file included from
<presumed file>:<presumed line>:` for each enclosing `#include`,
outermost first (`In included file:` under `-fno-show-source-location`).
The include location is updated for notes too, but a note never prints a
stack. Locations in the main file have no include
location, so returning to it prints nothing. Tested end to end against
clang 19's stderr in `crates/kiln_driver/tests/driver.rs`
(`include_stack_like_clang`).

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

## 11. Verify mode (`-verify`)

`kiln_diag::verify` is a port of clang 19's `VerifyDiagnosticConsumer`
(`clang/lib/Frontend/VerifyDiagnosticConsumer.cpp`), quirks included;
the driver side is spec/driver.md "-verify". Under `-verify` the
translation unit's diagnostics go to a `MemorySink`, not to stderr. The
`Verifier` collects `expected-*` directives from comments as they are
lexed. At the end, `Verifier::finish` reports marker errors,
`Verifier::check` matches the buffered diagnostics against the
directives, and the resulting `VerifyReport` is emitted through the
ordinary engine (text or JSON, §9-§10). Every report diagnostic is an
error located nowhere. After them the driver prints
`N error[s] generated.`, where N is the report's problem count, and
exits 1 if N > 0. It exits 0 when every diagnostic was matched, even if
those diagnostics were errors.

### 11.1 Comments

- The preprocessor hands every comment it lexes to the verifier (the
  comment handler, spec/pp.md): comments in the main file, in included
  files and in macro definitions. Comments in skipped conditional
  groups and in the raw text of `#error`/`#warning` are not handed over,
  because clang lexes those raw. Line splices (`\` newline) are folded
  out before parsing.
- Markers are scanned first. A `#name` (identifier characters) that
  starts a word marks the comment's line.
- Then directives are scanned. A directive is `<prefix>-<kind>[-re]`,
  where `<kind>` is `error`, `warning`, `remark`, `note` or
  `no-diagnostics`.
  - It must start a word: be at the comment start, or follow whitespace,
    or follow `//` or `/*`.
  - The token runs over `[A-Za-z0-9_-]` and then gives back trailing
    digits and `-`.
  - A token with an unknown prefix or kind is skipped, and the search
    continues after it.
  - One comment may hold several directives.
- With one prefix the search looks for the prefix itself. With several,
  it looks for any word and checks the prefix afterwards (clang's two
  paths behave the same).

### 11.2 Directive grammar

```
<prefix>-<kind>[-re][@<loc>] [<count>] {{<text>}}[{{...}}]
<prefix>-no-diagnostics
<loc>   ::= +N | -N | N | #marker | <file>:N | <file>:* | * | *:*
<count> ::= N | N+ | N-M | +
```

- **Prefixes.** The prefix is `expected` for plain `-verify`. With
  `-verify=a,b` it is any of the listed ones. A prefix is valid if it is
  a letter followed by letters, digits, `-` and `_`. Reports always name
  the alphabetically first prefix (clang's `*Prefixes.begin()`).
- **`no-diagnostics`.** It cannot be mixed with other directives
  (`diag_verify_invalid_no_diags`, either order). If the file has no
  directives at all, the report says "no expected directives found:
  consider use of '<prefix>-no-diagnostics'", which counts as one
  problem.
- **Locations.**
  - `@+N` and `@-N` are relative to the directive's line. `@N` is an
    absolute line.
  - `@<file>:N` finds `<file>` as a quoted `#include` written at the
    directive would. A missing file is `diag_verify_missing_file`.
  - `@<file>:*` and `@*` match any line, in that file or in the
    directive's file. `@*:*` also matches any file.
  - A line past the end of the file is clamped to the last byte
    (`translateLineCol`).
  - A malformed line is `diag_verify_missing_line`.
- **Counts.**
  - N exactly; `N+` at least N; `N-M` between N and M; a lone `+` one
    or more. The default is 1.
  - `N-M` with M < N is `diag_verify_invalid_range`.
  - A count of 0 is allowed. Such a directive consumes nothing, so a
    matching diagnostic stays unexpected.
- **Text.**
  - The text starts at `{{` (`diag_verify_missing_start` if absent).
    A string directive accepts any number of extra `{`.
  - The closing `}}` is searched with nesting
    (`diag_verify_missing_end`).
  - Several `{{...}}` in a row are concatenated.
  - `\n` in the text is a newline. Text after the last `\n` is then
    dropped, which is clang's quirk.
  - A `-re` directive with no `{{` in its text is
    `diag_verify_missing_regex`, and parsing of that comment stops.
- Directive errors are ordinary diagnostics located at the directive. If
  nothing expects them, they show up in the report as unexpected errors.

### 11.3 Markers

- `#name` defines a marker at the line of its comment. `@#name` locates
  a directive at that marker's line.
- A directive may name a marker defined later. It is attached when the
  marker appears.
- At `finish`:
  - A marker defined more than once and used is ambiguous:
    `diag_verify_ambiguous_marker`, with one
    `diag_verify_ambiguous_marker_here` note per definition.
  - A used marker that is never defined is `diag_verify_no_such_marker`,
    at the directive.

### 11.4 Regex

- A `-re` directive's text becomes one regex. Literal parts are escaped,
  and each `{{x}}` becomes `(x)`.
- The regex is a POSIX extended regular expression as `llvm::Regex`
  (Henry Spencer's `regcomp`) accepts it. The search is unanchored.
- A regex that fails to compile is `diag_verify_invalid_content`, with
  `llvm::regerror`'s text, and matches nothing.
- The engine is hand-written in `verify/regex.rs` because the `regex`
  crate is not a workspace dependency. It is a Pike VM, linear in the
  subject. It supports literals, `\c`, `.`, bracket expressions with
  classes, collating symbols and equivalence classes, `^`, `$`, groups,
  `|`, `*`, `+`, `?` and `{m,n}` with counts up to 255.
- A string directive matches if its text is a substring of the message.

### 11.5 Matching

- `check` flattens the buffered diagnostics, attached notes included, in
  emission order. Fatal errors count as errors.
- Levels are checked in the order error, warning, remark, note. Within
  a level each directive takes up to its count of diagnostics. A
  diagnostic matches if:
  - it is on the same presumed line (`#line` applies), unless the
    directive matches any line;
  - it is from the same file (clang `IsFromSameFile`): a macro location
    is replaced by its expansion location until the files agree; and
  - its text matches.
- A matched diagnostic is consumed, so each duplicate directive needs a
  diagnostic of its own. A directive short of its minimum count is
  listed once per missing occurrence. kiln caps the listing at 10000
  per level, which only matters for absurd counts.
- Diagnostics left over are unexpected, unless
  `-verify-ignore-unexpected` covers their level.
- kiln's engine deduplicates identical diagnostics (§7.4) and clang's
  does not. So two identical diagnostics at the same location are one
  diagnostic here; verify's own diagnostics are `allow_duplicate`.

### 11.6 Report

- For each level, the report first lists what was expected but not seen,
  then what was seen but not expected, as
  `diag_verify_inconsistent_diags`:
  `'<prefix>-<level>' diagnostics expected but not seen: ` (or
  `seen but not expected: `), followed by one entry per line.
- Expected entries read `\n  File <file> Line <n>[ (directive at <file>:<l>)]: <text>`.
  The parenthesis appears when the directive is not on the expected
  line.
- Unexpected entries read `\n  File <file> Line <n>: <message>`. A
  diagnostic located nowhere reads `\n  (frontend): <message>`.
- The ` File <file>` part is omitted for a diagnostic in a macro
  expansion, or in a buffer without a file (a name in `<...>`).
- The problem count is the number of entries, plus one for "no expected
  directives found".

### 11.7 Module diagnostics (WP-363)

The C++20 modules diagnostics follow clang 22's wording and locations,
not clang 19's (spec/modules.md §4 lists them by rule). The ids live in
the usual area tables: `pp_module_cycle`, `pp_module_expected_name`,
`pp_module_expected_semi`, `pp_module_not_found` (fatal, at the module
name) in `ids/pp.rs`; `sema_module_*` (preamble order, export placement and
linkage, imports, private fragment, `sema_module_redecl_attachment`) in
`ids/sema_e_m.rs`; `note_module_*` in `ids/note_m_z.rs`. Preamble errors
are reported at the `module` keyword, unnamed-namespace errors at the
namespace's `{`, and `export block begins here` is attached only for an
`export { ... }` block, as clang 22 does. Where clang 19 words a module
diagnostic differently (`... within a module purview`), the corpus test is
`requires: clang>=22`.

## 12. Deviations from clang 19

| # | Deviation | Why |
|---|---|---|
| D1 | No syntax highlighting of keywords/literals in colored snippets. | Needs a lexer in `kiln_diag`; cosmetic. |
| D2 | Adjacent fix-its are not merged (`mergeFixits`). | No caller needs it yet. |
| D3 | No word wrap / `-fmessage-length`. | The driver default is no wrapping. |
| D4 | `-fdiagnostics-color=auto` means off. | No terminal probing. |
| D5 | (Resolved by WP-1268: printability, formatting and width use LLVM 22's tables, §9.4.) Unicode version: UCD 15.1.0, as LLVM 22; a later LLVM's tables need a regenerated `unicode_tables.rs`. | |
| D6 | Unknown-option suggestions consider kiln's groups only, so e.g. `-Wbogus` suggests `-Wmost` where clang's larger table ties and suggests nothing, and `-Rbogus` suggests nothing where clang suggests `-Rpass`. | Kiln lacks clang's group membership data. |
| D7 | (Resolved by WP-203: the observable macro-argument rule is implemented, §8.4.) | |
| D8 | (Resolved by WP-056: the include stack is printed.) | |
| D9 | Selects are index-based (`{0:a\|b}`); there is no `%plural`. | Callers pass the index. |
| D10 | `-WX` is accepted as `-Werror`. | MSVC-style alias some build systems pass. |
| D11 | JSON is kiln's own schema (with `option`, `fixits`, severity `fatal`), not clang's SARIF. | Built for the test harness. |
| D12 | `pedantic` contains exactly the groupless extensions. | clang's `-Wpedantic` also has named subgroups kiln does not have yet. |
| D13 | *(Resolved by WP-183: `#pragma clang diagnostic` state, §5.5.)* | |
| D14 | `-verify` sees identical diagnostics at one location once (§7.4), so a test needing two of them with two directives fails under kiln (harness tests encode the known cases with `dup(clang)`, §7.4.1). | Engine deduplication; no libc++ test is known to depend on it. |
| D15 | A `-verify` report lists at most 10000 missing occurrences per level (§11.5). | Bounds the output of a count like `4000000000`. |
| D16 | P1467 extended floating-point diagnostics (WP-361) have no clang 19/22 counterpart; their texts follow GCC 14, adapted to kiln's style. `sema_fp_conversion_rank` (ExtWarn, group `float-conversion-rank`, a kiln-only name inside the `narrowing` umbrella; GCC: `-Wnarrowing`/`-Wpedantic`): "ISO C++ does not allow converting to {0} from {1} with {2:greater\|unordered} conversion rank", GCC's words, both types unqualified. `sema_arith_float_unordered` (Error): "invalid {arithmetic between\|bitwise operation between\|comparison of\|conditional expression between\|compound assignment to} floating-point types {1} and {2} with unordered conversion ranks", where GCC 14 says "invalid operands to binary + (have 'float' and 'float')" (its excess-precision types), "invalid operands to binary < (have '_Float16' and '__bf16')" and, for `?:`, "operands to '?:' of types '_Float16' and '__bf16' have unordered conversion rank". `lit_float_suffix_ext` (Extension, `c++23-extensions`): "'{0}' suffix on a floating literal is a C++23 extension"; GCC 14 accepts the suffixes silently before C++23. Tests: `tests/conformance/basic.extended.fp/p1467-conversions.cpp`, `p1467-suffix-ext.cpp`. | GCC 14 is the only reference implementation; spec/sema-conv.md §4, §9.13. |

## 13. Integration notes and open questions

- Lexer/preprocessor MUST pass `.range()` for underlines and MUST build
  expansion records as §8.5 describes; pasted/stringized tokens go in a
  buffer named `<scratch space>`.
- `kiln_source` could expose a line-start query; `kiln_diag` derives it
  from `line_text` pointer arithmetic.
