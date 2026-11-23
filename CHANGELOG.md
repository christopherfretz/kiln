# Changelog

All notable changes to kiln are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and kiln adheres to [Semantic Versioning](https://semver.org/). What a
version number promises is in [docs/STABILITY.md](docs/STABILITY.md).

## [Unreleased]

### Added

- `Ast::exception_spec_computed` (WP-2505, spec/ast.md §4.2): a tooling
  client tells an explicitly defaulted function's written exception
  specification (`S() noexcept(false) = default;`) from one computed
  from its subobjects, which has the same type once resolved; the new
  `FnSpec::exc_spec_computed` bit is set on every redeclaration when sema
  resolves a deferred specification, and PCH snapshots and module images
  (`IMAGE_VERSION` 83) carry it.
- Tooling locations (WP-2541, WP-2542): the row of an explicit
  instantiation in the declaration-extent table records the attributes
  written on it, each one's kind, syntax and extent
  (`DeclExtents::row_attrs`, spec/ast.md §19.2; carried by PCH snapshots
  and module images); the split `>` locations move onto `Ast`
  (`Ast::split_greaters`, carried by module images) and the public
  token-end query has a split-aware form (`kiln_lex::token_end_split`,
  `token_range_split`), so an expression ending at the first `>` of a
  `>>`, `>=`, `>>=` or `>>>` ends where sema says.

- Native stack sizing for evaluation-session workers (WP-2500,
  spec/eval-session.md §9 "Limits that bound the native depth"): every
  kiln limit that bounds how deep a run's semantic work recurses on the
  native stack (`-ftemplate-depth`, the Oven's call depth, its nested
  on-demand initializations, the parser's nesting), with its default,
  its option and the measured stack per level in debug and release
  builds (a reflection-driven class template chain: about 55 KiB and
  24 KiB per level, so a 4 MiB worker stops at level 46 or 118, long
  before `-ftemplate-depth=1024`), and how a client sizes a worker
  against them; the ignored test `measure_native_stack_per_level`
  re-measures it.

- The found set of a dependent written reference (WP-2472, spec/ast.md
  §18.12): for an unqualified dependent call `f(t)`, a parenthesized or
  qualified one, and a member of the current instantiation (`this->a(t)`,
  `C::a(t)`, `a(t)` with an implicit `this`), the declarations lookup
  found at the template definition, with flags for an open set
  (argument-dependent lookup adds to it), a dependent base and a
  dependent using-declarator. `WrittenRefs::found_at_definition`,
  `entries_finding` (rename in templates no specialization of which is
  instantiated) and `Ast::spec_origin` (the element a per-specialization
  row selected); `-fkiln-dump-written-refs` prints `def=` lines; carried
  by PCH snapshots and module images. Off, nothing changes.
- A per-run source overlay for tooling clients (WP-2390, spec/source.md
  "Overlay", stage 1): `kiln_source::Overlay` holds replacement bytes for
  files that exist, keyed by the identity of the file each path names
  (another spelling, a symbolic link or a hard link is the same file);
  `SourceManager::set_overlay` makes every load of an overlaid file take
  them (`FileOrigin::Overlay`, `original_bytes` the overlay's), `#embed`
  reads through it, and `FrontEndOptions::overlay` gives one to a
  front-end run, which then sees what the same command sees over a tree
  holding those bytes. A module file built from an overlaid file is
  refused (`drv_front_end_overlay_input` for one the command line names,
  `pp_module_file_overlaid` at an import).
- Evaluation sessions for tooling clients (WP-2412, WP-2413;
  spec/eval-session.md §2, §7): `SessionFrontEnd::consulted` lists the
  `#embed` resources and `__has_include` / `__has_embed` hits of the unit
  with their contents, as `FrontEnd::consulted`; an exception leaving a
  run's entry is reported with the call stack at its throw as `in call
  to` notes under `-fconstexpr-backtrace-limit=` (session runs only;
  compile output keeps clang's shape). `FrontEndOptions::response_files`
  (default on) turns off `@file` expansion in `run_front_end` (WP-2411).

- The source overlay creates and removes files (WP-2391, spec/source.md
  "Overlay", stage 2): `Overlay::create`, `remove` and `write`, keyed for
  created files by their deepest existing directory and the path below
  it; one resolver answers loads, header search (`kiln_pp::OverlayFs`:
  every `HeaderFs` query, listings with the created names, the search
  list) and the driver's input check, so a run sees what the same
  command sees over a copy of the tree with the edit applied;
  `Overlay::replace_with` / `create_with` take a loader called at most
  once per overlay, its failures in `FrontEndReport::overlay_failures`;
  `LookupLog::check` takes a `Trust` (replaying a log under another view
  asks every directory again). An overlay that creates any file refuses
  module files (`drv_front_end_overlay_creates`,
  `pp_module_file_overlay_creates`); a run without an overlay is
  unchanged.
- The written-reference table joins the AST and keeps the tokens
  outside a name (WP-2326, spec/ast.md §18.11): the location a
  `DeclRefExpr` or `MemberExpr` stores is its name's entry's `name_lo`
  (the `~` of a destructor, else `range.lo`), with the exceptions listed
  and a join test over the corpus; the leading `::` of a globally
  qualified name (`global_colon`, `qualified_lo`, also from a macro or
  across a comment or line splice) and a destructor's `~` (`tilde`) are
  recorded at parse in a cold side table, carried by PCH snapshots and
  module images, and dumped as `q=::L:C` and `tilde=L:C`.
- `__builtin_offsetof` keeps, for a non-dependent expression, its operand
  type and its resolved designator (per component the member, the base
  class stepped through, or the index position) in a cold row
  (`ExprArena::offsetof_designator`; PCH format 148), so tooling clients
  tell apart `offsetof(A, y)` and `offsetof(B, q)` at one offset. The ODR
  definition hash moves to scheme `kiln-odr-2`: it hashes that
  designator, a local class's tag, `#pragma pack` value and layout
  attributes, a local enumeration's underlying type, a coroutine lambda's
  wrapped body, a tuple-like structured binding's `get` call and a
  local's `cleanup(f)`, each a pair of definitions that differ in meaning
  and hashed alike before; `[[clang::musttail]]` and concatenated string
  literals are documented misses (WP-2360, WP-2361; spec/ast.md,
  spec/abi.md "Definition hash").

- The preprocessing record is carried in PCH snapshots and named by
  module imports (stage 4, WP-2113, WP-2260, WP-2300, WP-2301; spec/pp.md
  "Preprocessing record"): a PCH built with `-fkiln-pp-record` carries its
  header's record, and a recording consumer's record is that of the same
  compile with the header as `-include` (a PCH built without it leaves the
  record `restored`, with a `-v` note); a header unit's exported macros
  keep the unit's keys, so its import is complete; an import built from a
  module image is an `image` entry whose locations answer `Imported`. An
  `#embed` row records the resource read and its `--embed-dir` index, and
  `-fkiln-dump-pp-record=source-order` prints a condition's reads in
  source order. Off, a snapshot has no byte of the record (PCH
  `FORMAT_VERSION` 150).
- Written extents of expressions for tooling clients (WP-2327, WP-2328;
  spec/ast.md §21, spec/lex.md "Token ranges"):
  `kiln_ast::expr_extent::WrittenExtents` gives the first and last token
  of an expression exactly or `None`, and
  `kiln_parse::ExprExtents` maps it to bytes of one file by the rule the
  macro fix-its use (`kiln_lex::token_range`); `-fkiln-dump-expr-extents`
  prints them, and `cargo xtask extents` compares them with clang's source
  ranges, its stage 1 (the conformance corpus) a run of `ci`.
- The written-reference table's stage-4 gaps are closed (WP-2220..WP-2224,
  WP-2170; spec/ast.md §18.8, §18.9): per-specialization entries name the
  using-directive their lookup went through (`SpecRef::udir`), re-find
  dependent types rebuilt still dependent (generic lambdas, member
  templates), cover using-declarations with a dependent qualifier in class
  templates and the names they introduce, and record names in constraint
  satisfaction and requires-expressions keyed by the concept or the
  constrained template (cache hits keep the rows); `spec=` dump lines print
  canonical template arguments, so dumps agree whatever the order and
  spelling of instantiations; and the names in GNU thread-safety attribute
  arguments (`guarded_by(mu)`, `requires_capability(m)`) are looked up for
  the table, as clang parses them, without analysis. PCH FORMAT_VERSION
  145, IMAGE_VERSION 78.
- Declaration extents for module and import declarations, and for the
  declarations inside imported bodies (WP-2091, WP-2092; spec/ast.md
  §19.6, §19.9): every module-declaration and module-import-declaration
  has a row of its own kind (`ModuleDeclExtent`: `export`, the keyword,
  the module or header-unit name, the partition, the attributes and the
  `;`); an importer built from a module image has the rows a parse of the
  unit gives for the locals, lambdas, block-scope classes, structured
  bindings, init-statements and requires-expression parameters of the
  inline functions, inline variable initializers and template patterns
  it imports. The table gives back its spare capacity at the end of the
  parse. Off, nothing changes (PCH `FORMAT_VERSION` 146, `IMAGE_VERSION`
  79).
- Tooling-client API for evaluation sessions (WP-2320..WP-2325):
  `FrontEndOptions::written_refs` records the written-reference table in
  process (the table read after the closure equals the
  `-fkiln-dump-written-refs` dump); `kiln_diag::render_diagnostics` and
  `SessionFrontEnd::render` render `RunReport::diagnostics` in text or
  JSON through the driver's own sink (`kiln_diag::format_sink`), equal to
  the driver's standard error; `SessionFrontEnd` carries the unit's
  `diag_options` and `tu_diagnostics`; `RunError::Uncaught` gains
  `scalars`, the thrown object's first four integral or enumeration
  members (`UNCAUGHT_SCALARS`, spec/eval-session.md §8); `Ast: Send +
  Sync` is asserted at compile time. Under `-fkiln-reproducible-time`,
  `__TIMESTAMP__` is the pinned epoch's spelling in every file, whatever
  its modification time (spec/driver.md "Reproducible time").

- The written-reference table is carried in PCH snapshots and module
  images (stage 5, WP-1934; spec/ast.md §18.10): a TU restored from a
  snapshot made with the table continues its table (the dump equals the
  one with the header included as text), and an importer that records
  merges a module unit's entries from its image (the dump equals the one
  a replay of the unit gives). A prefix or module built without the
  table is listed as unrecorded (`WrittenRefs::unrecorded()`, the dump's
  first lines), never a silent gap. `-fkiln-written-refs` records
  without the dump (for a PCH or module build). Off, a snapshot grows by
  one byte and an image by four, and neither is read (PCH
  `FORMAT_VERSION` 144, `IMAGE_VERSION` 77).
- The preprocessing record's conditionals and inclusions (WP-2111,
  stage 2 of 4): one row per conditional directive, live or in a
  skipped group, with its outcome (taken, not taken, not evaluated,
  dead, closes), its condition and group ranges, its links to the rest
  of its conditional and its guard and unterminated flags; the
  `#pragma once` that took effect; one row per `#include`,
  `#include_next`, `#import`, header-unit `import`, `#embed` and
  `-include` with its outcome (entered, skipped by once or guard, not
  found, depth limit, ...), the file found and its search index; and
  `PpRecord::inclusion_of`, the include context of any location.
  `-fkiln-dump-pp-record` prints them as `cond`, `inc` and `once` lines,
  and every line in a header names its instance (`in i<n>`). No change
  with recording off (spec/pp.md "Preprocessing record").
- Evaluation-session results decode classes with virtual bases (each
  virtual base subobject once, at the most derived object, in
  `RecordValue::virtual_bases`, in initialization order; a pointer into
  one has a `PathStep::VirtualBase` step), and a pointer-to-member
  result gives its conversion path (`MemberPointerValue`: the member,
  clang's derived flag and the class chain), so `int B::*` and
  `int D::*` holding one member differ (WP-2210, WP-2212;
  spec/eval-session.md §4.1).
- The preprocessing record's stage 3 (WP-2112, stage 3 of 4): the
  operands of every paste and `#__VA_OPT__` and the argument of every
  `#x` (`PpRecord::made`); the reads of every evaluated condition
  (names tested, expansions entered, identifiers left as 0, each with
  the definition bound, and a partial flag when an error cut the
  condition off); the value and operand of every `__has_include` and
  feature query; every `push_macro`/`pop_macro` as an event paired with
  its push; and `enclosing`/`cond_at`, the innermost conditional group
  holding a location. `-fkiln-dump-pp-record` prints them as `made`,
  `read`, `query`, `push` and `pop` lines. No change with recording off
  (spec/pp.md "Preprocessing record").
- The written-reference table records what each dependent name of a
  template resolves to in every specialization (stage 4, WP-1933;
  spec/ast.md §18.9), in a separate table keyed by (pattern entry,
  specialization): member and qualified names, `typename`/`template`
  names and their dependent qualifier components, dependent bases'
  members, and unqualified dependent calls with their
  argument-dependent `how` and set, for function, member, variable and
  alias template instantiations, explicit instantiations, default
  arguments and generic lambdas. A substitution failure records
  nothing; a name an instantiation cannot resolve is `(unresolved)`.
  `-fkiln-dump-written-refs` prints the rows sorted after their entry
  (`spec=<specialization>`), and `specs_naming(d)` finds every
  instantiated use of a declaration for a rename.
- The written-reference table records declarative names (stage 3,
  WP-1932, WP-2100, WP-2131; spec/ast.md §18.4): using-declarators,
  using-directives, namespace alias targets, the names qualified
  declarator-ids and class-head-names redeclare (new role `redecl`),
  declaring elaborated-type-specifiers and friends, explicit and partial
  specialization and explicit instantiation targets, type-constraint
  concept names, field and offsetof designators and deduction-guide
  names, each with its lookup provenance; an identifier-accounting test
  (`crates/kiln_parse/tests/written_refs_accounting.rs`, `KILN_REFS_DIR`)
  checks that every identifier the parser sees is an entry, a
  declaration's own name or a documented non-name (spec/ast.md §18.8).
  The gaps it found are filled: `sizeof...(pack)` names, and
  declaration-extent rows for for-range and expansion-statement
  variables, condition and for-range bindings, exception declarations,
  requires-expression parameters and init-captures, with friend
  declarations no longer overwriting the row of what they name.
- Evaluation-session results of any result type (WP-2071,
  spec/eval-session.md §4.1): an entry may return a class, union, array,
  enumeration, floating-point, pointer, pointer-to-member or
  `std::meta::info` value, and `RunReport::result` is a `RunValue` tree
  the client owns (names as `display_string_of` spells them, `decl` only
  for what existed at open, objects without a name in a constants table
  `RunReport::constants` in first-reached order, one index per object),
  read through the freezer `constexpr R v = f();` uses and capped by the
  new `RunBudget::result_bytes` (64 MiB, `RunError::ResultTooLarge`).
  A `constexpr` entry's result is destroyed after the read. New
  `EvalSession::variable_value` / `variable_value_named` decode a
  constant variable's value through the same decoder. `RunValue` is no
  longer `Copy`; a class with a virtual base is refused for now
  (WP-2210).
- `constexpr` entries for evaluation sessions (WP-2072,
  spec/eval-session.md §3-§4): a session runs a parameterless
  `constexpr` function returning `void` or an integral type, not only a
  `consteval` one, by name or by `DeclId` (a function template
  specialization included). Its run is the manifestly
  constant-evaluated initialization of a notional `constexpr` variable
  at the end of the translation unit, so `if consteval` and
  `std::is_constant_evaluated()` are true and no declaration can be
  injected; a failure reports "call to constexpr function 'f' is not a
  constant expression" with the notes clang gives for
  `constexpr int v = f();`. A function escalated to immediate runs as an
  immediate invocation and is reported as one ("call to immediate
  function"). Budgets, isolation, determinism and replay are those of
  `consteval` entries. PCH snapshot format 139 (a new diagnostic id).
- The front end as a library call for tooling clients (WP-2120):
  `kiln_driver::front_end::run_front_end(argv0, command_line, options, f)`
  runs one `-fsyntax-only` job through the same driver stages as `kiln`
  and hands `f` the parsed unit (AST, Oven, source manager, preprocessor,
  the files `__has_include` / `__has_embed` / `#embed` consulted with
  their size and digest); nothing is written but diagnostics and the
  `import std;` module cache is read only. Options: a fixed
  `Environment` (no process variable is read), the PCH probe off.
  `front_end::std_config(std, options)` gives the configuration of
  `-std=<std>` with no other flag; the tables an `Ast` holds have
  (hidden) public paths.
- A working directory for the front end (WP-2121): `-working-directory
  <dir>` (clang's option) and `FrontEndOptions::working_directory` resolve
  every relative path of a compile (inputs, header search, `-include`,
  `#embed`, module files, PCH, `-o` / `-MF`, sanitizer lists) against a
  directory without changing the process's, so library entries in
  different directories can run at the same time; names as written
  (`__FILE__`, diagnostics, dependency files) stay relative.
  The command line's option wins over the library one.
- Location utilities for tooling clients that rewrite source text
  (WP-2150): `SourceManager::editable_range` / `editable_file_range` give
  the file characters a range written through macros stands for when
  they can be edited as one file range (clang's `makeFileCharRange`
  rules; fix-its now call the same function), `kiln_lex::token_end`
  lexes exactly one token to find its end, `SourceManager::expansion_entries`
  lists the macro-expansion entries with their parents, and
  `Preprocessor::add_callbacks` installs several preprocessor observers,
  called in registration order (spec/source.md, spec/lex.md, spec/pp.md).
- Two queries for tooling clients that read the `Ast` after the
  translation unit (WP-2025): `Ast::using_shadows(u)` gives the shadows a
  using-declaration introduced, in introduction order (namespace-, block-
  and class-scope using-declarations, `using enum`, inheriting
  constructors, and a dependent using-declaration's instantiations), and
  `Ast::shadow_using(s)` the reverse; `Ast::language_linkage(d)` gives a
  function's or variable's language linkage ([dcl.link]: C, C++ or none),
  from the one computation mangling and reflection now share (which also
  gives a block-scope `extern` declaration external linkage in
  reflection's `has_external_linkage`). Module images carry the link
  (image format 61); PCH snapshot format 127 (spec/ast.md §4.2, §4.5).
- A preprocessing record for tooling clients (WP-2110, stage 1 of 4):
  with `PpOptions::pp_record` (off by default) the preprocessor keeps
  every macro definition with its history (`#undef`, redefinition,
  `pop_macro`, `-D`/`-U`), each parameter's uses (plain, `#`, `##`), one
  entry per expansion with its parent and argument spans, and
  `PpRecord::origin`, which tells where any macro location came from
  (body token, argument token, pasted, stringized or builtin). Gaps are
  flagged, never silent (`record incomplete: restored` after a PCH).
  `-fkiln-dump-pp-record` prints it (with `-E`, the origin of each
  token), the library entry keeps it with `FrontEndOptions::pp_record`,
  and the test directive `pp-record:` checks it (spec/pp.md
  "Preprocessing record").
- Public tables for tooling clients that read the AST's untyped words
  (WP-2024, spec/ast.md §20): `kiln_ast::roles`, an operand-role row for
  every type, expression, statement and attribute kind and every cold
  side map (which the module image's attribute checks now derive from,
  with the image format unchanged); `kiln_snap::Describe` descriptors
  for the borrowed views (`TypeView`, `ArgView`, `ConstValue`, ...);
  and stable names for the `FuncInfo` layout and SME bits, the
  floating-point option bits, the address spaces and the atomic
  builtin ids.
- Two facts overload resolution and instantiation read now stay in the
  `Ast` after the translation unit, for tooling clients (WP-2020,
  WP-2021): an overloaded member access's qualifier, explicit template
  arguments and naming class (`Ast::member_access_name`, kept whether or
  not a call resolves the access), and a dependent operator expression's
  definition-context lookup set (`Ast::dependent_oper_candidates`, sets
  interned by content), which sema now reads from there at
  instantiation (spec/ast.md §5.10, §8.5). PCH snapshot format 122.
- A closure owner query for tooling clients (WP-2023):
  `Ast::closure_owner(record)` names the innermost declaration whose
  source text holds a lambda-expression (the template parameter, the
  templated declaration of a requires-clause, the alias, the function,
  the parameter, the variable or field), an instantiated closure's being
  the instantiated declaration. Kept only where it differs from the
  closure's context, and carried by PCH snapshots and module images.
- A checked marker on attribute nodes for tooling clients (WP-2022):
  `AttrNode::CHECKED` (flags bit 6) says the checks of the declaration
  or statement the attribute appertains to have run on it, so its
  argument words have their checked meaning (a `cleanup`'s `a` is the
  function's `DeclId`, not the argument's `ExprId`). Query it with
  `AttrNode::is_checked` / `AttrArena::is_checked`. After the
  translation unit every attribute a declaration or an attributed
  statement holds is checked; implicit attributes are built checked,
  and PCH snapshots and module images carry the bit in the existing
  flags byte (no format change).
- Lookup provenance on the written-reference table for tooling clients
  (WP-2130): each entry now says how lookup found the name
  (`WrittenRef::how`: unqualified, qualified, member, argument-dependent
  only, both, builtin, injected-class-name, none), the using-directive an
  unqualified or qualified lookup went through
  (`WrittenRefs::using_directive`, the innermost link of a transitive
  chain), and, for a call subject to argument-dependent lookup, the
  associated namespaces, classes and enumerations it searched
  (`WrittenRefs::adl_set`, interned). The entry stays 28 bytes; the rest
  is a sparse side table. `-fkiln-dump-written-refs` prints the new
  fields; under the same switch, no cost when off (spec/ast.md §18.7).
- A declaration-extent table for tooling clients (WP-2090):
  `-fkiln-decl-extents` records, for every declaration as written, its
  full range from its first token (an attribute, a template head, a
  braceless `extern "C"`) to its `;` or `}`, its own declarator range in
  a declarator group, its name and qualifier, its body (with
  ctor-initializers and handlers, or `= default`/`= delete`/`= 0`) and
  where its decl-specifiers end (`Ast::decl_extent`);
  `-fkiln-dump-decl-extents` prints them. Off by default; carried in PCH
  snapshots (format 129) and module images (version 62)
  (spec/ast.md §19).
- A written-reference table for tooling clients (WP-1930, stage 1):
  `-fkiln-dump-written-refs` records, for each expression name, member
  name and nested-name-specifier component written in the source, the
  declaration it denotes (the overload resolution winner for an
  overloaded name, none for a dependent one), what lookup found, the
  enclosing declaration and the qualifier to its left, and prints one
  line per name. Stage 2 (WP-1931) adds type names in every
  type-specifier context (casts, `new`, `sizeof`, handlers, template
  arguments, typename-specifiers, elaborated references, `using enum`,
  pseudo-destructors), template names (template-ids, CTAD, template
  template arguments, `.template` members), base-specifiers and
  mem-initializer-ids, with the using-shadow as `found` when a
  using-declaration brought the name in. Off by default, with no cost
  beyond one branch per name; not carried in PCH snapshots or module
  images yet (spec/ast.md §18).
- Host calls for tooling clients (WP-1990, spec/host-calls.md): a
  program embedding kiln can register a Rust handler (`HostCalls`, with
  `HostOut` and `HostError`, through `parse_preprocessed_with_host`)
  that constant evaluation calls through
  `__builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, cap_out,
  bout, cap_bout)`. The builtin exists only with a handler registered.
  Its ranges are checked before the handler runs, it is never memoized,
  a handler error ends the evaluation with its message, and a call in a
  speculative evaluation or at run time is an error. No precompiled
  header or module file is written with a handler. PCH snapshot format
  120.
- Retained evaluation sessions for tooling clients (WP-1995,
  spec/eval-session.md): after a translation unit, `EvalSession` (opened
  from a parsed `Sema`, or from a driver command line with
  `kiln_driver::session::with_eval_session`) runs a named parameterless
  `consteval` entry any number of times. Each run starts from the
  finished translation unit's state and nothing it does reaches the
  next; it has its own host-call handler, step, memory and depth budget
  (a program profile by default) and a cancellation token checked from
  any thread, and returns its value, cost counters and diagnostics
  (with call-stack notes, not printed). Steps and diagnostics do not
  depend on what earlier runs memoized or instantiated, every failure
  leaves the session usable, and the memory it keeps stays flat over
  runs. A translation unit with errors is refused. PCH snapshot format
  128 (a new driver diagnostic id).
- Header units' external definitions (WP-1800): while a header unit is
  built (by `-fmodule-header` or at an import), a definition of a
  non-inline function or variable with external linkage is now clang's
  "non-inline external definitions are not permitted in C++ header units"
  error ([module.import]/6), with clang's exemptions (no initializer,
  `extern`, inline, constexpr, templated, instantiated; `weak` and
  `selectany` from clang 22). PCH snapshot format 108.
- Header units in the conformance harness (WP-1802): two-step tests and
  `--bmi-kind` / `--bmi-image` runs now give kiln header-unit module
  files built with `-fmodule-header` (`header-unit:` tests are selected
  by `--bmi-kind`), `two-step` accepts `header-unit:` lines, and the new
  `as-header-unit` directive builds the test file itself as a header unit.

- The Arm FP8 types and intrinsics (WP-1791), as clang 22 has them:
  `__mfp8` (`mfloat8_t`), `__SVMfloat8_t` (`svmfloat8_t`) and its x2/x3/x4
  tuples, NEON vectors of `__mfp8`, and all 325 FP8 SVE and SME builtins
  (247 + 78) with `fpm_t` operands, lowered as clang 22 lowers them. The
  types follow clang 22's rules (sizeless SVE vector, opaque byte with no
  arithmetic or conversions, its mangling, calling convention, RTTI and
  debug info); at `-fclang-compat=19` they do not exist, as in clang 19.
  Also fixed on the way: SVE vector `/`, `%` and `>>` on signed element
  types are now `sdiv`, `srem` and `ashr`, as clang emits them (kiln
  emitted the unsigned operations).
- clang 22's `<arm_sve.h>`, `<arm_sme.h>` and `<arm_vector_types.h>`
  (WP-1842): at the default `-fclang-compat=22` the driver searches
  `<resource>/include-clang22` first, so clang 22-only SVE/SME intrinsics
  (the FP8 ones, `__arm_in_streaming_mode` as a builtin) and the
  `mfloat8_t` / `fpm_t` helpers are declared; `-fclang-compat=19` keeps
  clang 19's headers. The install tree carries the new directory.

- Template heads in module interface exposures (WP-1855): a template
  that is not TU-local whose template-head names a TU-local entity (a
  type-constraint's concept, also `Hid auto` and a placeholder's in a
  variable's type, a requires-clause naming a concept, function, type or
  variable, a template parameter's type or default argument, a lambda's
  head) is now a "declaration of 'f' exposes TU-local entity" error
  ([basic.link]/14), as GCC 16.1 rejects; also for partial
  specializations and deduction guides. It was accepted.
- Deduced types in module interface exposures (WP-1856): a variable whose
  placeholder type, or a non-inline function whose return type, is
  deduced to a TU-local type, or to a local class or closure type of a
  TU-local specialization, is now an exposure error ([basic.link]/15.1,
  15.2, 16.5), as GCC 16.1 rejects. It was accepted.
- Imported templates re-check nested function-type parameters (WP-1859):
  the module image now carries the written parameters of function types
  inside declarators, so substituting an imported template rejects an
  address space or Swift parameter ABI the arguments make invalid, as
  after a replay. The image version is now 47.
- Concept definitions in module interface exposures (WP-1845): a
  concept that is not TU-local whose constraint-expression names a
  TU-local entity (a function, also in `sizeof` or a requires-expression,
  a type, a TU-local concept, a use in a lambda there) is now a
  "declaration of 'C' exposes TU-local entity" error, as GCC 16.1 rejects
  it ([basic.link]/14); a concept-name denoting a TU-local concept now
  names it in an inline function too. A constant read and an enumerator
  stay valid (GCC 16.1 rejects those).
- Local classes of function template specializations in module interface
  exposures (WP-1846): a specialization of a non-inline function template
  whose deduced return type names a local class or closure type naming a
  TU-local entity (in a member, a base, a constexpr member function's
  body, a returned lambda; also through argument-dependent lookup in the
  instantiation) is TU-local ([basic.link]/15.5), so an inline function
  using it is now an exposure error naming the specialization; it was
  accepted. GCC 16.1 rejects some of these at the instantiated member.
- Folded full-expressions in module interface exposures (WP-1847): a use
  of a TU-local entity is no longer dropped when its full-expression
  folds (`(void)helper;`, `0 ? helper() : 1`, `&K != nullptr` in an
  inline function or a default member initializer): [basic.link]/14
  names it, so each is now an exposure error. GCC 16.1 folds these away
  and accepts them (a documented deviation).
- Overload sets in module interface exposures (WP-1819): an
  id-expression names every function of its overload set
  ([basic.link]/14.5), so a set holding a TU-local function that overload
  resolution does not select (found by lookup or argument-dependent
  lookup, a function template, a template-id, `operator+(a, b)`) is now a
  "declaration of 'f' exposes TU-local entity" error in an inline
  function, a declaration's type or default argument and a default member
  initializer; an operator expression's candidates are not. GCC 16.1 and
  clang 22 accept these (a documented deviation).
- Local classes of non-inline functions in module interfaces (WP-1820):
  one rule. Such a class is part of the ignored body whatever its
  members' specifiers (a constexpr member function naming a TU-local
  entity is no longer an error), except a class the function's deduced
  return type names, whose declarations are all checked, as GCC 16.1.
  An importer calling a non-inline member function of such a class
  through the deduced type now links (it emits its own internal
  definition, as clang).
- Unevaluated operands, discarded statements and declarators in module
  interface exposures (WP-1821): naming a TU-local entity in `sizeof`,
  `decltype`, `noexcept`, a requires-expression or an `if constexpr`
  branch not taken in an inline function, or in a declarator's
  `decltype(helper()) f();`, is now an error, as [basic.link]/14 names
  them (clang 22 warns, GCC 16.1 accepts). An enumerator or a
  specialization that is not TU-local used there, and a direct use of an
  inline template specialization whose body names only such entities,
  get clang 22's `-WTU-local-entity-exposure` warning.
- The last TU-local exposures kiln accepted silently in a module
  interface (WP-1814, GAP-174): a local class's base, data member or
  member function signature naming a TU-local entity in an inline
  function, and a use of an inline template's instantiation whose body
  names one (an argument-dependent call in the instantiation too,
  [basic.link]/16.5) are now "declaration of 'f' exposes TU-local entity"
  errors, as GCC 16.1 rejects them. An enumerator of a TU-local
  enumeration, a non-inline local-class member's body and a non-inline
  function template's specialization are not exposures by the text and
  get clang 22's `-WTU-local-entity-exposure` warning.
- clang address spaces (WP-1803): `address_space(N)` and the `opencl_*`
  attributes (`__attribute__` and `[[clang::]]`) are a type qualifier, as
  in clang: part of the type for overloading, templates and deduction,
  converted implicitly only into a space that holds the source (a
  C-style cast converts any), mangled `U3AS<N>` / `U8CLglobal`, lowered
  to `ptr addrspace(N)`, `addrspace(N)` globals and `addrspacecast`, and
  transparent to constant evaluation. They were "not supported by kiln
  yet".
- A value-dependent `address_space(N)` in a template (WP-1811, WP-1812,
  WP-1813): clang's `DependentAddressSpaceType`, with its checks at
  instantiation, deduction of N, the mangling `U2ASI <expression> E` and
  modules and PCH carrying it; a space after a grouping `(` of a
  declarator qualifies the type at the parentheses (a function type
  there is now the clang error); an alias's decl-specifier space no
  longer qualifies the whole type; tests for x86's `__seg_gs` /
  `__seg_fs`.
- clang's C++ spelling of the `_BitInt` literal suffix, `__wb` / `__uwb`
  (WP-1797): `1__wb` is a `_BitInt(2)`, in expressions and in `#if`, with
  clang's pedantic `-Wbit-int-extension` warning.
- `#include <arm_sme.h>` compiles (WP-1794): `__builtin_arm_get_sme_state`
  is declared and lowered as clang does, a call of the SME ABI routine
  `__arm_sme_state`, so `__arm_has_sme()` and `__arm_in_streaming_mode()`
  work on any AArch64 host.
- Every known-incomplete language corner is listed in
  docs/status/static-gaps.md and a lint keeps the list exact (WP-1719).
- Function multiversioning on x86-64, as clang 22 does it (WP-1745):
  `target("default")` beside other `target` versions, `target_clones`
  and `cpu_dispatch` / `cpu_specific` compile to per-version functions
  and an `ifunc` whose resolver picks one from the running CPU, with
  clang's redeclaration checks and diagnostics. On AArch64 a non-default
  `target_version` member or friend is now a specific error.
- The `swiftcall` and `swiftasynccall` calling conventions (WP-1745):
  `swiftcc` / `swifttailcc` with clang's Swift argument lowering on
  x86-64 and AArch64 (classes split into at most four register values,
  vectors into legal vectors), `musttail` for a `swiftasynccall`
  function's returned `swiftasynccall` call, and the "variadic function
  cannot use X calling convention" error (also for `regcall` and
  `vectorcall`).
- The Swift parameter attributes `swift_context`,
  `swift_async_context`, `swift_error_result` and
  `swift_indirect_result` (WP-1745), as clang: part of the function
  type (and its mangling), clang's position, type and calling
  convention checks, `swiftself` / `swiftasync` / `swifterror` /
  `noalias` (+`sret`) in IR, and the `swifterror` temporaries on both
  sides of a call. A calling convention among a type-id's
  decl-specifiers (`using F = __attribute__((swiftcall)) void (*)()`)
  is no longer dropped.
- x86 `interrupt` handlers (WP-1745): `x86_intrcc` definitions with
  clang's signature checks, the error for a direct call and
  `-Wexcessive-regsave`.
- The x86-64 GNU `model("small" | "large")` variable attribute
  (WP-1745, clang 22) and clang's `weak_import` (WP-1745: `extern_weak`
  declarations, warnings on definitions).
- Pack expansions in the contexts that reported "not yet implemented"
  (WP-1733): a mem-initializer pack expansion of a member class
  template's own pack, a using-declarator pack expansion at block scope
  and of a conversion function, a
  base-specifier pack naming a generic lambda's pack in a local class, a
  pack expansion nested in a partly substituted pattern
  (`T... x, decltype(x)... y`), deduction against partly substituted pack
  expansions, and a type-constraint of a concept whose last parameter is a
  pack (`C<T> auto`).
- A pack named outside an expansion in a template's declarations is
  diagnosed when the template is defined, as clang does: a base type, an
  enumerator value, a using-declaration, a bit-field width, a fixed
  underlying type, an explicit- or noexcept-specifier, a mem-initializer
  and an attribute argument (WP-1733).
- Class templates instantiate members that used to stop with "not yet
  implemented" (WP-1734): member deduction guides ([temp.deduct.guide]),
  `using enum` members ([enum.udecl]), and member and friend templates
  whose own non-type or template template parameters appear in their
  signatures or noexcept specifiers ([temp.mem], [temp.friend]).
- Initialization and conversion corners that reported "not supported
  yet" now follow the standard (WP-1735): `void{}` (CWG2351), braced
  placement arguments of a new-expression (`new ({1, 2}) T`), an empty
  braced list calling a constructor template as the default constructor,
  a conditional operator over two xvalues of differently cv-qualified
  types, and a defaulted assignment operator whose subobject assignment
  needs overload resolution.
- Dependent names that reported "not yet implemented" (WP-1737): member
  friends named by an operator (`friend void *A<T>::operator new(...)`),
  qualifiers that stay dependent inside generic lambdas and their local
  classes, deduction guides of alias templates and inherited
  constructors whose parameter types name members of the class template,
  qualified destructor names looked up at instantiation
  (`p->NS::S::~Q()`, `p.T::~Q()`), and `D::template f<A>` through a
  dependent using-declarator ([temp.dep.type], [over.match.class.deduct],
  [basic.lookup.qual.general]/4).
- The GNU `x ?: y` operator on class types (WP-1738): `x` is evaluated
  once (a class prvalue materialized), converted to `bool` by its
  conversion function, and the result is the conditional of [expr.cond]
  with `x` as its second operand, in code generation and in constant
  evaluation.

- clang 21-22 builtins (WP-1740): `__builtin_structured_binding_size`,
  `__builtin_{lt,le,gt,ge}_synthesizes_from_spaceship`,
  `__builtin_invoke` and `__builtin_common_type`; `__has_builtin`
  answers 1 for them at clang compatibility 22, so libc++ 22 takes its
  builtin paths.
- A function template declared through a typedef, alias or `decltype`
  of a function type (`template<class T> F f;`, [dcl.fct]/13) can be
  specialized and called (WP-1740).
- Code generation paths that reported "not supported yet" (WP-1739):
  the builtins the frontend accepted but could not lower (the AMX
  `*_internal` and Key Locker builtins, MMX `maskmovq`, AArch32
  `__a32_vcvt_bf16_f32`, `__atomic_always_lock_free` with a size known
  only at run time); compound assignment, `++` and `--` of `_Atomic long
  double` on x86-64; C23 `= {}` of a variable length array; constants of
  classes with a `[[no_unique_address]]` member that overlaps its
  neighbours (laid out as clang does); `typeid`, throw and handlers of
  `_BitInt` and `_Atomic` types; the caller-side postcondition check of
  a function returning an `_Atomic` class type; an under-aligned `byval`
  argument (copied to an aligned temporary).
- More code generation paths that reported "not supported yet"
  (WP-1739): the GNU `x ?: y` operator on `_Complex` operands, `_Atomic
  _Complex` objects (loaded and stored whole and atomically, also through
  the `__c11_atomic_*` builtins), a `co_await` or `co_yield` whose result
  is `_Complex`, a `_Complex` conditional with a throw-expression arm (as
  GCC accepts it), and a C structure assignment used as an arm of `?:`.
  The remaining exhaustive arms of the code generation visitors name the
  invariant that keeps them unreachable.
- AAPCS64 pure scalable types (classes and arrays of
  `arm_sve_vector_bits` vectors and predicates) and SVE tuple types
  (`svint8x2_t`) can be passed and returned by value at
  `-fclang-compat=20` and later, in SVE registers as clang 20+ does (by
  reference once the registers run out or for a variadic argument);
  kiln reported "passing or returning 'T' by value is not supported yet"
  (WP-1739).
- Every SVE, SVE2 and SME ACLE intrinsic (`<arm_sve.h>`, `<arm_sme.h>`,
  `<arm_neon_sve_bridge.h>`) is lowered as clang lowers it: clang 22's
  8987 builtins from `-fclang-compat=20`, clang 19's 8628 below, IR equal
  to clang's, read off clang's own output by
  `scripts/gen-sve-builtins.py` into a generated table; all but 720 of them
  reported "code generation for SVE builtin ... is not supported yet".
  An alias of a name that is not one of these builtins is "can only be
  applied to an ARM builtin", as in clang; the FP8 intrinsics wait for
  the FP8 types (WP-1739).
- `-verify`, the record-layout, vtable-layout and CFG dumps work with
  every action (WP-1745): a codegen job checks its diagnostics and
  writes its output only without errors, and prints the dumps before its
  IR; `-E` verifies the preprocessor's diagnostics. `-fdump-ast` is an
  action, as clang's `-ast-dump`, and `-###` prints the step of `-E`,
  `-M` and `-fsyntax-only` jobs.
- GNU variable length array types outside a function (WP-1745):
  `sizeof(int[n])` at namespace or class scope is evaluated at run time,
  and a function template's parameter may be variably modified. In C++
  no variable length array is formed in a SFINAE context ("variable
  length array cannot be formed in a SFINAE context"): a non-constant
  bound in a requirement or a deduced signature is a substitution failure.
- `#pragma clang section bss|data|rodata|relro|text="name"` (WP-1744):
  variable definitions with static or thread storage duration get the
  current data sections as LLVM global attributes, function definitions
  the text section; section type conflicts with a pragma are diagnosed.
- `#pragma options align=k` and `#pragma align=k` (`native`, `natural`,
  `packed`, `power`, `reset`) share `#pragma pack`'s stack (WP-1744).
  `#pragma pack(show)` prints the value, and the `#pragma pack` forms
  clang ignores (an invalid alignment, `pop` on an empty stack, an
  unknown action) get clang's warnings instead of an error. A push
  never popped is reported at the end of the file (`-Wpragma-pack`).
- Before C++20, `export template <...>` is accepted with clang's
  "exported templates are unsupported" warning ([diff.cpp03.temp]), and
  any other `export` is clang's "expected template" error, instead of the
  "modules is not supported yet" error (WP-1744).
- Header units as module files (WP-1744): `-fmodule-header`,
  `-fmodule-header=user|system`, `-x c++-user-header`,
  `-x c++-system-header` and `-x c++-header-unit-header` write a header
  unit's module file, and `-fmodule-file=h.pcm` imports it at
  `import "h.h";` / `import <h.h>;`, as clang does. `kiln-scan-deps`
  scans header-unit imports (it used to reject them), and `-E` / `-M`
  perform them for their macros and print them as clang 22 does.
- Module directives follow clang outside the usual places (WP-1744):
  before C++20 `module` and `import` lines are ordinary declarations, a
  module declaration or import after a `#pragma` that the parser handles
  is accepted, `-E`/`-M` of a module file is clang's "previously
  preprocessed input" warning, and `-fdump-ast` of a module file
  dumps its unit.
- The floating-point environment pragmas (WP-1744): `#pragma STDC
  FENV_ACCESS ON`, `#pragma clang fp exceptions(maytrap|strict)`,
  `#pragma float_control(except, on)` and `#pragma STDC FENV_ROUND
  FE_TONEAREST|FE_DYNAMIC` make the function `strictfp` and its FP
  operations `llvm.experimental.constrained.*` calls with the scope's
  rounding and exception metadata, as clang does; constant expressions
  are still evaluated in the default environment. clang's "is illegal
  when precise is disabled" family of errors is reported.
- Directed rounding modes and evaluation methods (WP-1744): `#pragma
  STDC FENV_ROUND FE_UPWARD|FE_DOWNWARD|FE_TOWARDZERO|FE_TONEARESTFROMZERO`
  emits clang 22's constrained intrinsics with the mode, and constant
  evaluation rounds every operation, conversion and literal in the mode
  written at it, in every float format (functions and templates keep
  their definition's mode). `#pragma clang fp eval_method(double|extended)`
  promotes lower-ranked operands as clang does, in code and in constant
  evaluation, with clang's `__FLT_EVAL_METHOD__` and fast-math errors.
- The floating-point environment flags (WP-1798): `-frounding-math`,
  `-ftrapping-math`, `-ffp-exception-behavior=maytrap|strict`,
  `-ffp-model=strict` and `-ffp-eval-method=double|extended` are no
  longer errors. They set the unit's rounding, exception and evaluation
  state, emitting clang 22's IR (`strictfp` functions and constrained
  intrinsics), its `__FLT_EVAL_METHOD__` and its `option 'ffp-eval-method'
  cannot be used with ...` errors.
- A function with many nested blocks or a long chain of `const` locals
  no longer takes minutes to compile when its floating-point environment
  is dynamic (`-frounding-math`, or `FENV_ACCESS ON` in a block): the
  check that keeps such code from being folded is now linear (WP-1798).
- Constant evaluation of the expressions that reported "not supported
  yet" (WP-1741): a `case` label nested in a statement of its switch
  body (Duff's device), the taken branch of an `if constexpr` whose
  discarded branch names an uncaptured entity, a discarded built-in
  `<=>`, an array new-expression initialized by a string literal,
  `std::initializer_list` with two pointer members, and in C an
  assignment's value used as a structure's initializer. A `(T&)x` cast
  that performs a reinterpret_cast is reported as clang does, and an
  `std::initializer_list` of another layout is an error ("layout not
  recognized"; the PCH format is now 78).
- Constant evaluation of the builtins on a `_BitInt` wider than 128 bits
  (WP-1741, WP-1742): the checked arithmetic builtins
  (`__builtin_add_overflow` and the rest, wrapping the result as at run
  time), `__builtin_popcountg`/`clzg`/`ctzg`/`ffsg`/`parityg`/`clrsbg`, the
  element-wise builtins, `__builtin_align_up`/`_down`/`__builtin_is_aligned`
  with a wide value or alignment, `__builtin_constant_p`, and a wide
  `new T[n]` size.
- Before C++26, constant evaluation of `delete` of a `void *` destroys the
  allocation's dynamic type, as clang does (WP-1741).
- Constant evaluation of vectors of `_BitInt` lanes wider than 128 bits
  (`_BitInt(256) __attribute__((ext_vector_type(2)))`; WP-1742): the
  operators, lane subscripts and writes, element access,
  `__builtin_convertvector`, shuffles, splats, reductions and element-wise
  builtins; a narrow vector shifted by a wide count. The PCH format is
  now 84.
- Constant evaluation of a `_BitInt` wider than 128 bits as a subscript,
  a pointer offset (`p + s`, `p += s`), the shift count of a narrow left
  operand (`1 << s`, `x <<= s`) and the operand of an integral to pointer
  conversion, and of `.hi`/`.odd` of an odd-sized ext vector prvalue,
  whose lane past the end holds no value (WP-1742).
- Reflection corners that reported "not implemented" or "not supported
  yet" (WP-1743): `constant_of` of a `_BitInt` member or element wider
  than 128 bits, `reflect_constant` of such a value, splices of their
  value reflections, and splices of an object reflection of the real or
  imaginary part of a complex object (`reflect_object(__real__ z)`; also
  a reference template argument designating one, which used to stop with
  an internal error). Every value and object reflection now splices; the
  `std::meta` traits and the query family have no unimplemented row.
- `std::meta::constant_of` of an explicit-object member function
  (`void f(this S&)`, also a conversion function, an operator and a
  member function template specialization) is the function itself, and
  `extract` of an explicit-object conversion function gives its pointer
  (WP-1782).
- The static-gap lint also reads the spec: a `spec/*.md` paragraph led by
  "Not implemented" or "Unsupported" must be listed in
  docs/status/static-gaps.md, for gaps with no message of their own in
  the code (WP-1782).
- `-fconstexpr-profile-split` (WP-1864): a sampled split of the
  constant evaluator's time by phase (instruction dispatch, checks, the
  memory model, the memo table, sema-bridge callbacks, call frames,
  compiling to bytecode), one more `constexpr: split` line after the
  `-fconstexpr-stats` summary. It is measured by a kiln built with the
  `kiln_oven/split` feature; any other build reports it unavailable and
  pays nothing for it (spec/driver.md "Constexpr profile", spec/oven.md
  "Cost split").
- Promoted complex arithmetic (WP-1795, GAP-172):
  `-fcomplex-arithmetic=promoted`, and `-ffp-model=fast`, which selects
  it, no longer stop with "not supported by kiln". A complex division is
  done in the next wider floating type with the algebraic formula
  (Smith's algorithm, and clang 22's end-of-file
  `-Whigher-precision-for-complex-division` warning, when there is no
  wider type), as clang 22 does on both targets. The complex-range flags'
  interactions with `-ffp-model=`, `-f[no-]fast-math` and `-Ofast` now
  follow clang 22, with its `-Woverriding-complex-range` warnings, and
  clang 19 at `-fclang-compat=19`, with its `-Woverriding-option`
  warnings.
- Sanitizer coverage (WP-1863): `-fsanitize-coverage=` (every clang
  feature: `trace-pc-guard`, `inline-8bit-counters`, `trace-cmp`,
  `pc-table`, `stack-depth`, `control-flow`, ...),
  `-fno-sanitize-coverage=`, `-fsanitize-coverage-allowlist=`,
  `-fsanitize-coverage-ignorelist=` and clang 22's
  `-fsanitize-coverage-stack-depth-callback-min=` no longer stop with
  "not supported by kiln". They combine with `-fsanitize=fuzzer` as in clang, with its
  deprecated spellings, warnings and errors, and the instrumentation
  matches clang 22's (clang 19's at `-fclang-compat=19`).
  `no_sanitize("coverage")` and `__has_feature(coverage_sanitizer)` work.
  The fuzzer set now also traces indirect calls, as clang's does, and
  `-fsanitize=fuzzer`'s `"no-builtin-<name>"` attributes are clang's.
  `-fno-builtin-<name>` now drops a name clang does not know as a
  builtin (`-fno-builtin-memmem` gives no attribute), as clang does.
- CI misbehaviour checks (WP-1901): a new `guard` job on the aarch64
  lane runs a corpus slice (run tests, links, modules) under strace
  (`cargo xtask guard trace`) and fails when any process opens a socket
  or writes outside the target directory. On a pull request it first
  annotates the changes that run code at build time or change CI (new
  or changed dependencies in `Cargo.toml`/`Cargo.lock`, a new `build.rs`
  or proc-macro crate, `.github/`, `.cargo/`) with `::warning::` lines
  and a job summary (`cargo xtask guard risky`; it does not fail). Every
  workflow now has a read-only default token (`permissions: contents:
  read`).
- Tier-0 CI for outside pull requests (WP-1902): `.github/workflows/tier0.yml`
  runs `cargo xtask ci --step lints` and the workspace's library unit
  tests on x86_64, for pull requests from forks only, with a read-only
  token, no secrets and a timeout on every job (docs/VALIDATION.md
  "Tier-0 (outside pull requests)", spec/harness.md "CI").
- `-mgeneral-regs-only` on AArch64 and x86-64 (WP-1804), as clang 22
  and 19: AArch64 disables `fp` and what depends on it (`neon`, `sve`,
  `crypto`, ...) after the `-march=` / `-mcpu=` extensions, and a
  function or call that passes or returns a floating-point, vector or
  HFA value is the error "requires 'T' type support, but ABI 'aapcs'
  does not support it"; x86-64 adds `-x87,-mmx,-sse` where the flag
  stands among the `-m` feature flags, so the SSE and MMX macros go and
  each use of a `long double` declaration is "requires  'long double'
  type support". The predefined macros and `"target-features"` follow.
  The flag was an unknown argument.

### Changed

- `kiln_ast::dump::AstNames::for_keys` / `type_key` print a type as a
  key: unlike the diagnostic text, two different types never print alike
  (a typed null pointer argument keeps its type, a local class its
  enclosing function and a block ordinal, an unnamed class or closure its
  location; spec/types.md §12.1). `oper_lookup::SetSpan` is public, with
  `OperLookups::span`. The unused `DeclMisc` module-ownership bits are
  removed (the owner is `ModuleTable::owner`) (WP-2477, WP-2545).

- Breaking, for tooling clients (WP-2480, WP-2502, WP-2500, WP-2503;
  spec/eval-session.md §8, §9): what a failed evaluation-session run
  hands over. `RunError` moved from `kiln_oven` to `kiln_sema::session`
  (still re-exported by `kiln_parse`; the Oven's own outcome is now
  `kiln_oven::RunFault`). `RunError::Uncaught`'s `type_name: String`
  is replaced by `ty: Entity`, the exception type spelled fully
  qualified (`ns::v0::error`, as `display_string_of` spells it) with
  its class's first declaration when it existed at open, and it gains
  `value: Result<Box<ValueTree>, String>`, the thrown object's value as a
  result would be read (or why it has none). A host call refused by
  kiln's check of its arguments is the new `RunError::HostArgument {
  message }` (the handler is not called); `RunError::Host` is now only
  the handler's own error. A run that exhausts the calling thread's
  stack is the new `RunError::StackExhausted` (was `Failed`), also when
  it reaches work an earlier exhaustion stopped. New
  `RunError::is_host()`: an unknown kind is kiln's failure of the run,
  never the host's.
- More expressions have a written extent for tooling clients (WP-2350..
  WP-2353, spec/ast.md §21): a dependent template-id keeps its `>` and an
  operator-function name its end (`this->f<T>`, `T::template v<int>`,
  `operator==(a, b)`, `x.operator()<1>`); a qualified name through an
  implicit `this` (`B::f`) and a qualified concept-id (`ns::C<T>`) keep
  their qualifier's begin; a pseudo-destructor keeps the end of its type
  (`p->~T`), a designated initializer its designator (`.x = 1`), a
  parenthesized aggregate initialization its parentheses (`S s(1, 2)`);
  and a substituted template argument maps to the parameter's name. The
  locations live in the cold maps only; a designator keeps its `.`, so
  PCH snapshots change format (`FORMAT_VERSION` 153).

- Breaking, for tooling clients (WP-2411, spec/eval-session.md §2):
  `with_eval_session` takes its command line as words with no `@file`
  response-file expansion; an `@x` word is an ordinary word (an input
  named `@x`).

- The front-end library entry's environment (WP-2122, spec/driver.md
  "Front-end library entry"): `KILN_WREF_RAW` (the written-reference
  dump's raw locations), `KILN_STATS_LOG`, `KILN_HOD_STRICT`,
  `KILN_PCH_SNAPSHOT_PASSES` and the crash reproducer's `TMPDIR` are read
  from the driver's environment (`FrontEndOptions::environment` for a
  library run) instead of the process's; the remaining process-wide
  developer switches are listed in one table.
- Breaking, for tooling clients (WP-2321, WP-2322, WP-2324, WP-2325):
  `HostCalls` no longer requires `Send`; only a registered handler is
  `Box<dyn HostCalls + Send>`, and `EvalSession::run` takes any handler
  (an `Rc` one included). `SessionRefused` says why no session was
  opened: `CommandLine { message }` (the driver's message),
  `TuHasErrors { errors, diagnostics }` (the records, not only a count),
  `Stopped { diagnostics }` (a fatal error, or no tree) and
  `Panicked(text)` (a front-end panic, caught). `RunError::Uncaught` has
  the new field `scalars`. A host call in work a run triggers is refused
  with the note "a host call cannot be evaluated here: this expression
  is evaluated when the code is instantiated, not when it runs", and
  every run that reaches the work fails identically.
- The decoded preprocessor-state record `kiln_pp::state::PpRecord` is
  renamed `kiln_pp::state::StateRecord` (WP-2114), so it no longer
  shares its name with the preprocessing record `kiln_pp::PpRecord`.
- `EntryError::NotConsteval` is renamed `EntryError::NotConstexpr`
  (WP-2072): it is now returned only for a function that is neither
  `constexpr` nor `consteval`.
- Evaluation sessions charge nested work to the run (WP-2073,
  spec/eval-session.md §5.1): constant evaluations that semantic work
  needs during a run (an instantiation's `static_assert`, a static
  member's initializer, a reflection query) run to completion under the
  translation unit's limits, are recorded once per unit, and are charged
  to every run that reaches them. `RunReport` gains `nested_steps` and
  `nested_calls`; `steps` and `calls` stay the run's own. The step, byte
  and depth budgets apply to the run's own plus its charged nested cost,
  so a run that does the work and one that replays it get the same
  counts, budget decision and diagnostics. A host call in nested work
  is refused with the note "host call in semantic work a run triggered
  (its result is kept across runs)". Fixed: a memoized call could skip
  a replay of diagnostics; work shared by two units was replayed with
  the first only; an initializer read under a small run budget kept its
  failure for later runs. PCH snapshot format version 135.

- Peak memory since 1.1.0 measured and explained (WP-1891,
  docs/status/perf.md "Peak RSS since 1.1.0 (WP-1891)"): kiln's peak RSS
  is 1.4-5.1 MiB higher than 1.1.0's. Nearly all of the rise is kiln's own
  code pages mapped into the process (`.text` grew from 12.0 to 16.2 MB),
  and the heap of a small compile grew by at most 0.6 MiB. No table,
  cache or buffer is allocated earlier or larger than in 1.1.0. spec/bench.md
  now says that the bench's peak RSS includes those code pages.

- Faster constant evaluation of standard-library string plumbing
  (WP-1864): `std::string_view` comparisons over many `std::string`s
  run 4-5x faster on libstdc++ (an N=4300 all-pairs comparison: 32 s
  to 6 s). The Oven now reads the evaluation context for a call of
  `std::is_constant_evaluated` or the library's own wrapper of it without
  running the call, evaluates `std::char_traits<C>::compare`, `length`
  and `find` natively (declining, so the library's code runs and
  diagnoses, whenever a read would be invalid or a limit would be
  crossed), caches the element extent of an array inside a union
  (std::string's small buffer), and keeps two union paths per
  allocation in its path memo. A call of one of these library functions
  now costs fewer `-fconstexpr-steps` than in clang: none for the
  wrapper, one per element examined for a bulk operation (spec/oven.md
  "Library fast paths"); user code is counted as before.
- Fewer constant-evaluation allocations for scalar locals (WP-1908).
  An argument of `std::move`, `std::forward`, `std::forward_like`,
  `std::move_if_noexcept` or `std::as_const` no longer forces the local
  into the Oven's abstract memory. Neither does a scalar declared
  without an initializer and assigned before any other use. Both now
  stay in VM registers, and the libc++ `std::sort` bench allocates 55%
  fewer bytes. Steps, values and diagnostics do not change. The
  `-fconstexpr-stats` memory counters (`bytes`, `peak-live`,
  `reclaimed`, `peak-arena`) drop where such locals were in memory
  (spec/oven.md "Register locals").
- Cheaper constant-evaluation calls (WP-1907, WP-1917, WP-1918). A call
  of a function that makes no frame object (a light frame) skips the
  Oven's memory mark, frame serial and release. `std::min` and
  `std::max` of two arithmetic values (with no comparator, or a
  `std::less`/`std::greater` one) and `std::char_traits<C>::assign(c1,
  c2)` are evaluated natively, declining to the library call whenever it
  would diagnose or a limit would be crossed; read for its value, a
  `std::min`/`std::max` leaves its locals in VM registers and allocates
  no temporary. The `string-view-find-if` bench runs 7.8% faster with
  22% fewer bytes. Each such call now costs one `-fconstexpr-steps` step
  (spec/oven.md "Library fast paths", "Light frames"); light frames
  change no step count.
- Inlined constant-evaluation calls of small leaf functions (WP-1924).
  At its first execution, a direct call of a short function that makes
  no call, no frame object and no allocation and is not memoized (an
  accessor, `operator[]`, an iterator step) is replaced by a copy of the
  callee's body in the caller's code. That saves about 80 instructions
  per call. Steps, the depth limit, values, the memo and every
  diagnostic, with its "in call to" note, are unchanged. The bench's
  steps, calls and bytes are identical. Evaluation time drops 1.6% for
  `string-view-find-if`, 3.4% for `sort` and 7.5% for `table`
  (spec/oven.md "Inlining"). PCH snapshot format 116.
- The bench and status numbers are re-measured (WP-1886): `docs/status/perf.md`
  is now against clang 22.1.8 (it was clang 19), and the new
  `docs/status/perf-gcc.md` holds the same corpus against GCC 16.1
  (`cargo xtask bench --compare-with g++-16 --record` writes it). Bench
  records and pages write a measured compiler's path relative to the
  checkout (`target/release/kiln++`) or as its name, never a local
  absolute path, and `cargo xtask ci --step lints` rejects a local
  absolute path in any tracked file. `scripts/env.sh` takes
  `KILN_TARGET_ROOT` to move the per-checkout target directories.
- The release archives are kiln's profile-guided build (WP-1862):
  `release.yml` runs `cargo xtask dist --pgo --check` on aarch64 and
  x86_64, and the release gate builds and tests that same binary (its
  record names the profile; `--no-pgo` is the escape hatch for a
  re-run). On the held-out bench set the PGO binary takes 0.82x the time
  of the plain one on both aarch64 and x86_64, with 0.96x to 0.97x the
  memory ([docs/status/perf.md](docs/status/perf.md) "PGO build"). `cargo xtask dist` writes `build-info.txt` beside the
  archives (plain or PGO, the profile's sha256), and the sweep workflow
  has an opt-in `pgo` measurement job. The count of functions without
  profile data no longer puts build scripts and proc macros in kiln's
  share when cargo prints in color (as on CI runners), nor the proc
  macro `kiln_snap_derive` at all. `cargo build --release` and
  `cargo xtask dist` without `--pgo` still build plain; the packager's
  path is unchanged (`cargo xtask pgo-profile`, README.md "For
  packagers").

### Fixed

- A global `operator new` or `operator delete` declared after a class
  template whose pattern befriends it is one function with the implicit
  declaration and the instantiated friend, before or after the
  instantiation (WP-2540): it was chained to the pattern's friend, which
  declares nothing, and a new-expression could call that friend. A friend
  allocation function declares the implicit ones (so a `::`-qualified
  friend `operator new[]` is valid), and a friend `main` in a class
  template no longer makes `int main(int, char **)` a conflicting
  declaration.
- An unqualified `v<...>` that ordinary lookup does not find, in an
  explicit instantiation or specialization, an elaborated type or a
  friend, no longer declares a class `v` (WP-2547): it is clang's "no
  template named 'v'; did you mean 'a::v'?" followed by the
  namespace-placement error for `a::v`, or "explicit instantiation of
  undeclared template struct 'w'" when no template resembles it. An
  explicit instantiation whose class-key does not match the template is
  a tag mismatch, and `union w;` after `struct w;` redeclares the same
  class rather than making `w` ambiguous.
- A statement expression in a default member initializer or a default
  argument of a local class (WP-2474, spec/sema.md §5.8): its locals and
  local classes belong to the enclosing function (clang's mangling,
  `_ZZ1fvE1s`), and the statement expression may use them, capture
  them in a lambda (`[&]` is a local lambda there) and declare
  `static` ones; they were rejected as references to a local of the
  enclosing function. A namespace-scope class's default member
  initializer keeps clang's `statement expression not allowed at file
  scope`.
- An elaborated-type-specifier in a local class member function's
  parameter list (`void g(struct Q *);`) declares `Q` in the enclosing
  block scope (WP-2506, [basic.scope.pdecl]/7.2), so the later
  block-scope definition redeclares it: one redeclaration chain, and
  `a.g(&q)` type-checks.
- `clone_with_children` copies a `LambdaExpr` (WP-2507, spec/ast.md):
  the copy gets its own lambda record (a new capture-initializer list)
  and shares the closure type, call operator and body; it returned
  `None`, so a rebuilt expression holding a lambda was not copied.

- Five expression shapes that had no written extent for tooling clients
  now have clang's (WP-2460..WP-2464, spec/ast.md §21): an implicit
  `this->operator T*` in a class template ends at the conversion type's
  last token; a member named by a dependent using-declarator (`using
  T::f;` then `f()`) begins at its name; `&B::f` of an overloaded member
  and a static member template's specialization through an implicit
  `this` (`s<0>`, `B::s<1>`) begin at the qualifier; an array `new`'s
  parenthesized initialization (`new int[2](1, 2)`) begins at its `(`;
  and `&this->sf<int>` ends at its `>`. `cargo xtask extents` compares
  them all with clang 19. No node, image or snapshot change.
- Parameter attributes follow clang across declarations and templates
  (WP-2476): a redeclaration's parameter inherits the previous
  declaration's `nonnull`, `annotate`, `carries_dependency` and Swift ABI
  attributes (`lifetimebound` too from clang 22), shown `Inherited` by
  `-fdump-ast`, so a definition gets a prior declaration's `nonnull` and
  `llvm.var.annotation`; a call's `-fsanitize=nonnull-attribute` check,
  `maybe_undef` freeze and `noescape` read the declaration the call
  names, not every redeclaration; in C++ declarations that differ in a
  parameter's `noescape` are `conflicting types` (C forms the composite
  type); a template's parameter attributes are all instantiated (only the
  Swift ABI ones were), with a dependent `align_value(N)` checked at
  instantiation (a substitution failure when not a power of two); the
  parameters of a function type inside a declarator have their attributes
  checked; `aligned(N)` on a parameter aligns its storage; `align_value`
  on a non-static data member is ignored with a warning, as clang, instead
  of accepted.
- Parse cost that grew faster than the input in shapes tooling clients
  meet (WP-2478): a run of labels and of case labels (the label and
  case-value searches are hashed, the case-range overlap checks
  binary-search), an `else if` chain (the lookahead memo stamp hashed
  every open scope and every lookup and every `return` walked all of
  them: the scope stack now keeps a revision and a walk index that skips
  empty block scopes), nested classes (a class-head name that cannot be
  qualified is no longer looked up through every enclosing class, and the
  [temp.local]/7 search no longer rescans the scopes below each class
  scope). All now scale linearly; 100 000 labels parse in 0.08 s instead
  of 5.9 s.
- A designated initializer list nested N deep in a template keeps N + 1
  written lists, not (N + 1)(N + 2)/2: the written forms of the inner
  lists are shared (WP-2478).
- Under `-fclang-compat=22`, a class nested in a local class or in an
  unnamed class prints in a canonical diagnostic, `__PRETTY_FUNCTION__`
  and a failed requirement as clang 22 prints it: with its enclosing
  function and parameter types (`store<n::f(int, ...)::Local::In>`) and
  `O::(unnamed struct)::In`; clang 19's form skips an unnamed enclosing
  class (`O::In`). A closure in a failed `static_assert` requirement
  prints `(lambda at file:line:col)` under both releases (WP-2545).
- A `_` member of an anonymous union or anonymous struct is no longer
  marked name-independent ([basic.scope.scope]/5); an ambiguous use of it
  keeps clang's placeholder wording (WP-2477).
- The attributes of an explicit instantiation (WP-2541): the
  decl-specifier ones are merged onto the specialization as clang merges
  them, so `template __attribute__((weak)) void f<long>(long);` defines
  a `weak` function, `extern template __attribute__((weak)) ...`
  declares it `extern_weak`, a `weak` variable instantiation is `weak`,
  and `deprecated` warns at the uses; one after the declarator is
  dropped, as clang drops it. A standard attribute before the
  decl-specifiers, or after the class-key of a class's explicit
  instantiation, is "an attribute list cannot appear here". A `weak` (or
  other interposable) function definition is no longer inferred
  `nounwind` (clang `TryMarkNoThrow`).
- A user-defined literal's call, and an expression it begins, begin at
  the literal's token rather than at its suffix; a concatenated string
  literal's call ends at its last piece (WP-2543).
- `#embed` data has a place in the unit (WP-2544): its scratch buffer is
  an expansion of the directive, so a diagnostic in the embedded data
  points at the `#embed` line as clang's does, and tooling clients can
  map every embedded token back to its directive.

- `EvalSession::variable_value` instantiates on demand a static data
  member's initializer that nothing instantiated (an in-class `constexpr`
  member of a class template specialization, an out-of-line `const` one),
  as spec/eval-session.md §4.1 says, instead of answering `NotAConstant`;
  it checks the calling thread's stack guard, and a later run that
  reaches an initializer it failed reports that failure as in any session
  (WP-2420).
- A `SessionRefused` for a unit with errors or a stopped one carries its
  diagnostics rendered as the driver printed them (`rendered`, and
  `SessionRefused::rendered()` for every case), text a client can use
  after `with_eval_session` returns, when the records' source manager is
  gone; the two variants are `#[non_exhaustive]` (WP-2332).
- A read of a `constexpr` variable whose initializer failed at a host
  call no longer reports a second error, "host call during speculative
  evaluation", from re-folding the initializer: in a translation unit and
  in a refused nested host call of an evaluation-session run (WP-2330,
  spec/host-calls.md §5).
- `__TIMESTAMP__` without a fixed epoch is the current file's
  modification time in local time (the C library's zone: `TZ`, else
  `/etc/localtime`), as clang spells it (`"Sun Nov 22 21:05:00 2026"`),
  instead of `??? ??? ?? ??:??:?? ????`; standard input has time 0, as in
  clang (WP-2331, spec/pp.md "Builtin macros").
- The declaration-extent table (`-fkiln-decl-extents`) gives every
  written declaration its own row (WP-2470, spec/ast.md §19.1): an
  explicit instantiation of a function template, a variable template, a
  member function or a static data member (`template void S<long>::m();`,
  `template void t(short);`) has one, and an `extern template`
  declaration and the later explicit instantiation definition, a
  repeated explicit or partial specialization declaration and a repeated
  opaque enumeration declaration no longer overwrite the earlier
  declaration's row: rows are keyed by declaration and ordinal
  (`DeclExtents::rows_of`). The written-reference table records every
  member name of an `offsetof` designator (WP-2471, §18.4): the names
  after a component that failed are `UNRESOLVED`, and a template's
  designator deferred to instantiation resolves the names into a type
  that is not dependent. PCH format 152, module image 81.
- An evaluation-session run on a thread other than the one that
  compiled the session checks that thread's stack guard (WP-2410,
  spec/eval-session.md §9): it failed "stack exhausted" at its first
  instantiation or nested evaluation, and every later run of the session
  with it. A stack exhausted by one run no longer stops the next.

- The written-reference table records a failed lookup as `UNRESOLVED`,
  not `DEPENDENT` (WP-2291, spec/ast.md §18.2): an undeclared identifier
  (also in a thread-safety attribute's argument), a member name after an
  invalid object expression, and in a template pattern a non-dependent
  name or a non-dependent call's callee whose lookup or resolution fails
  at the definition; the identifier-accounting test counts the arguments
  of attributes clang does not know as non-names, leaving nothing over
  in `tests/conformance` (WP-2290).
- A class template's friend function template definition with a default
  template argument, instantiated after the template was declared at
  namespace scope, is "default template argument not permitted on a
  friend template" ([temp.param]/12), as clang; kiln accepted it. And
  `va_arg` of a class that is not trivially copyable keeps its
  `VAArgExpr` but marks it erroneous while `-Wnon-pod-varargs` is an
  error, so a consumer walking the body sees it (WP-2362, WP-2363,
  spec/sema.md §5.12).

- An alias-declaration (or alias template) whose type-id is invalid
  declares nothing, as clang: later uses of its name are "unknown type
  name". An enum-base whose type is invalid gives the enumeration the
  underlying type `int`. Before, an enumeration whose base was such an
  alias kept the error type, and a constant expression building one of
  its values (`E{1}`) aborted the constant evaluator with an internal
  compiler error, a panic through the front-end library entry (WP-2173,
  WP-2141, spec/sema.md §1.2).
- Evaluation sessions record a class template specialization's
  instantiation (with its member classes and enumerations), an exception
  specification instantiated or computed outside the Oven's `noexcept`
  query, a cached constraint satisfaction and atomic constraint, and a
  default member initializer as units of their own (WP-2075,
  spec/eval-session.md §5.1): a run that reaches such work done by an
  earlier unit, through a different instantiation, a `noexcept`
  operator, another constrained template or constructor, or a
  reflection query, is charged its `nested_steps`/`nested_calls` and
  shown its diagnostics, as a fresh session's run is.
- A chain of N member calls (`x.f().f()...`, `p->f()->f()...`, by value
  with a destructor), subscripts or overloaded `<<` compiles in time
  linear in N, also when constant-evaluated or lowered with `-g`
  (WP-2201, reported by a downstream user): each call walked the chain
  below it to find where it begins, in sema, the Oven and codegen.
  `x.f()` repeated 20,000 times took 23.6 s at `-fsyntax-only`.
  Locations, diagnostics and dumps are unchanged.
- Evaluation sessions: a member function of a class template
  specialization instantiated by another function's instantiation and
  compiled later in the same run is now recorded with that
  instantiation (WP-2072, spec/eval-session.md §7), so a later run that
  calls it first is charged its nested work and replays its diagnostics,
  as a fresh session's run is.
- Constant pointers and references to two members of one union at the
  same offset with the same type (`&u.a` and `&u.b` of `union { int a;
  int b; } u`) are distinct (WP-2200, spec/ast.md §9.1 "Union paths"):
  as template arguments they are different arguments ([temp.type]/2.4),
  mangle as clang 22 does (`soiL_Z1uE_0E`), print as the member they
  name and substitute as it (a read of the inactive one is diagnosed);
  this holds through nested unions, array members, `void *` round trips,
  pointers formed in a constant evaluation and pointers held by a class
  value. A pointer that enters no union costs nothing more. PCH format
  141, module image 74, carried-constants section version 5. Reported
  by a downstream user.
- Evaluation sessions key the diagnostics a reflection query records by
  the content of its operands, not by their addresses (WP-2074,
  spec/eval-session.md §7): a `substitute` over a vector a run built, a
  `define_static_string` or a filtered `members_of` replays its
  warnings in a later run that builds the same content elsewhere, and a
  run that builds other content at the same address no longer gets the
  earlier run's warnings. The spec lists every key and cache a run
  reads or fills and why none holds an address. No change without a
  session.
- A function given a reflection of an automatic variable (`f(^^x)`) and
  reading it with `extract` or `constant_of` is no longer memoized by the
  reflection's bits (WP-2074): a second call after `x` changed returned
  the first call's value, in any constant evaluation and across the runs
  of an evaluation session.
- `std::meta::extract` and `constant_of` of a structured binding
  introduced in the running constant evaluation (`auto [a, b] = P{1, 2};
  extract<int>(^^a)`, a reference or tuple-like decomposition too), and
  `constant_of` of a local variable or parameter of a live frame, are
  constants, read as the frame holds them, also in a function the
  reflection was passed to (WP-2161, [meta.reflection.extract]): they
  failed with "the value of 'a' is not usable in a constant expression".
- A parenthesised call of `std::min` or `std::max` read for its value,
  `1ull << (std::min((size_t)31, n))`, no longer stops the Oven with an
  internal error ("expected a call, found ParenExpr") (WP-2061): the
  native fast path looked through the parentheses for the arguments and
  then expected a bare call. Reduced from LLVM 19.
- An imported template's dependent operator expression under
  `-fkiln-bmi-image` (WP-2021): the declaration image did not carry the
  operator functions the template's definition found, so the importer's
  instantiation looked the operator up again and saw the module's
  operators declared after the definition (choosing a later, better
  `operator-`, or accepting an operator the definition could not see).
  The image now carries the set (`IMAGE_VERSION` 56).
- The attributes of an enumerator and of a lambda's call operator are
  now checked against them as any declaration's (WP-2022): kiln attached
  them as written, so an attribute that does not apply there was kept
  without a diagnostic (`nodiscard`, `alignas`, `enable_if` or
  `noreturn` on an enumerator; `nonnull` naming the implicit object
  parameter or out of bounds, `alloc_size` on a deduced return type,
  `tls_model`, an unknown `visibility` on a call operator), as clang
  reports them. A deduction guide's pushed `#pragma clang attribute`
  attributes are checked too.
- An array new-expression with a bound known only at run time as the
  operand of an assignment, a conditional or a comma (`p = new S[n];`
  with `S` a class with a `constexpr` constructor) no longer crashes the
  -Winteger-overflow check, which evaluated the element constructor as
  an expression of its own; the check now stops at such a new as clang
  does, so overflows in its initializer no longer warn (reduced from
  LLVM 19, WP-2060).
- An instantiated access to a member of an anonymous union or struct
  (`ct->a` in a function template, `ct` a pointer parameter) now keeps
  the lvalue-to-rvalue conversion of the pointer and the derived-to-base
  cast of the object, as clang's AST does; kiln dropped them, which
  tooling clients read and the constant evaluator crashed on (a
  bit-field assignment `t->a = ct->a`; reduced from LLVM 19, WP-2060).

- An ordinary unqualified call whose function only argument-dependent
  lookup found (a hidden friend, a function or function template of an
  associated namespace, an ADL-only template-id) now carries the `adl`
  flag on its `CallExpr`, dumped `adl` as clang's `IsADLCandidate`; kiln
  set it only on operator calls and structured-binding `get` calls, so
  tooling clients reading the flag missed these calls. Codegen, constant
  evaluation and diagnostics are unchanged (WP-1928).
- The arguments of an attribute clang knows but kiln does not interpret
  are now parsed and counted as clang does, from a table generated from
  clang 19 and 22 (`scripts/gen-attr-args.py`,
  `docs/status/attr-args.tsv`): `__attribute__((cold(1)))`,
  `[[gnu::used(1)]]`, `alloc_size(1, 2, 3)` and about 200 others are now
  clang's "'x' attribute takes no arguments" (or "takes one argument",
  "requires exactly N arguments", "takes at least / no more than N")
  error, a non-string at a string position, a pack expansion and a
  missing `)` are clang's errors, and the attribute is dropped; kiln
  skipped these clauses as balanced tokens and accepted them (WP-1879).
- An attribute whose argument is invalid (`cold(undeclared)`) is now
  dropped with that error alone, while one whose argument recovered
  (`leaf(f())`, `f` undeclared) is still checked; `visibility`,
  `type_visibility` and `pcs` take a leading identifier and say
  "attribute requires a string" as clang does; every GNU attribute before
  a free-standing `struct` (unknown ones, clang 19's `gcc_struct`, a
  wrong count) is the "place it after struct" warning; clang 22's
  `nvptx_kernel` argument is parsed as an expression (WP-1880..1883). PCH
  snapshot format 117.

- The "expression evaluates to 'L == R'" note of a failed `static_assert`
  evaluates the operands manifestly constant-evaluated, as the condition
  was, so `if consteval` and `std::is_constant_evaluated()` take the same
  branch there (`'1001 == 5'`, not `'1 == 5'`); so do the unsatisfied
  constraint notes that print operand values or pick the false operands
  of a `&&` (WP-1943).
- A failed constant evaluation that computes `a + 4` over `int a[3]`
  before a one-past-the-end read is reported at the pointer arithmetic,
  the first operation that is not constant ([expr.add]/4); clang reports
  the later read. Recorded in docs/status/deviations.md (WP-1944).
- The "exceeded maximum depth" note of a member call (`r.get()`,
  `p->get()`) or an overloaded operator call (`a + b`, `a == b`) is at the
  member name or the operator, as clang puts it, and so is the "non-constexpr
  function"/"undefined function" note of an overloaded operator call;
  kiln put them at the call's first token (WP-1942).
- An address space written over a template parameter or a dependent
  `address_space(N)` type (`T __attribute__((address_space(1)))`) whose
  substituted type is already in another space is now clang's
  "conflicting address space qualifiers are provided between types"
  error at the type as written (an alias template's at its `using`), and
  a substitution failure in SFINAE; kiln silently replaced the space
  (WP-1914).
- Under a file-scope `#pragma STDC FENV_ROUND`, a constructor's member,
  base and delegating initializers are now emitted under the FP state
  they were written in, and a default member initializer under its own,
  as clang does: kiln used the command line's rounding there, so their
  constrained operations carried `round.tonearest` (WP-1904). PCH
  snapshot format 114.
- A default argument is now emitted under the FP pragma state it was
  written in (an in-class member's, an inherited one's and a template's
  included), as clang does: kiln used the calling function's, so under
  `#pragma STDC FENV_ROUND` its constrained operations carried the
  caller's rounding (WP-1919).
- Under kiln's C++26 default fill, an aggregate local whose initializer
  writes every byte (no padding, every member given, defaulted or
  value-initialized: `Agg h{.b = y}`) no longer gets a redundant
  `llvm.memset` before it, so its IR is clang 22's; padded types, unions,
  constructors and a written `-ftrivial-auto-var-init=zero` keep the fill
  (WP-1921).
- A module interface with blocks or initializers under a floating-point
  pragma (`FENV_ROUND`, `#pragma clang fp`) is now imported from its
  declaration image: the image declined those declarations and the
  importer replayed the module (WP-1923). Image format 52.
- `C::C<args>` and `C::C` after a non-dependent qualifier naming the
  class `C` (`L::mapping<int>::mapping<long> *p`) are now clang's
  "qualified reference to 'mapping' is a constructor name rather than a
  template name" error, and with a preceding `typename` or `template`
  its -Winjected-class-name warning, as for a dependent qualifier; a
  base-specifier, a mem-initializer and a class-key still name the class
  (WP-1903).
- Diagnostic wording, order and dumps (WP-1906: WP-1809, WP-1816, WP-1822,
  WP-1844, WP-1848, WP-1849, WP-1850, WP-1851, WP-1854, WP-1860), now as
  clang 22: `-Woverriding-option` warnings for the FP flags; the
  `-Wundef-prefix=` warning; typed value arguments in instantiation notes
  (`dep<2UL>`); the "attribute declaration must precede definition"
  warning before redefinition errors; "expected function body after
  function declarator" at `-std=c++98`; unknown-attribute warnings after
  the attribute list's syntax errors, and clang's wording for a missing
  attribute token; "in instantiation of requirement here" notes; the
  narrowing note for a deduced array bound; vtable-dump method names of
  class template specializations (`void A<int>::f(T) [T = int]`); and
  `MustTailAttr`, `NoMergeAttr` and written explicit-specialization
  arguments in the AST dump.
- Builtin required-feature expressions (WP-1841): the NEON, ACLE and x86
  builtin checks now use clang's evaluator (`|` loosest, then `,`,
  parentheses group), shared with SVE and SME, so
  `__builtin_ia32_vpmadd52huq128` (`avx512ifma,avx512vl|avxifma`) and
  similar builtins are accepted with only `avxifma`; they were rejected.
- `[[deprecated()]]` and `[[nodiscard()]]`, an empty argument clause on a
  standard attribute that takes an optional one, were accepted (WP-1852);
  they are now clang's error "parentheses must be omitted if 'x'
  attribute's argument list is empty" and the attribute is dropped. The
  no-argument check ("attribute 'x' cannot have an argument list") now
  covers the `__x__` spellings (`[[__noreturn__()]]`) and
  `[[clang::fallthrough()]]` too, and in C only the attributes C has. As
  in clang, the clause is parsed first, so an invalid one reports only its
  own error (`[[deprecated(x)]]` is "expected string literal as argument
  of 'deprecated' attribute", not an undeclared `x`).
- The arguments of an `__attribute__` clang's parser does not know are
  parsed by clang's unknown-attribute rule even when kiln has its own
  table for the name (WP-1853): `__attribute__((nodiscard(x)))` and, at
  `-fclang-compat=19`, `gcc_struct(x)` / `coro_await_elidable(x)` are only
  "unknown attribute" warnings, no longer "use of undeclared identifier".
  Conversely an attribute clang knows for another target (`long_call`,
  `signal`, `dllimport`) has its arguments parsed as expressions, as
  clang does.
- The non-`constexpr` reference case of P2686R5 (a local reference to an
  automatic object, `int& r = x;`, constant-initialized by the C++26 text,
  so `std::is_constant_evaluated()` would be true in its initializer) is
  now a recorded deliberate deviation (WP-1830): kiln keeps clang's and
  GCC's run-time behaviour, pinned by run tests against both.
- A calling convention written as the target's default (WP-1858):
  `__attribute__((sysv_abi))` or `cdecl` on x86-64, `cdecl` on AArch64,
  and an ignored convention that warns and becomes the default, now count
  as explicit, as in clang: `ms_abi void f(); sysv_abi void f();` is the
  "function declared 'cdecl' here was previously declared 'ms_abi'"
  error (it was accepted), a second convention on the same declaration
  conflicts with it, and the type prints as written
  (`'void () __attribute__((sysv_abi))'`). The type stays the type
  without the convention.
- Swift parameter ABIs in template argument deduction (WP-1857):
  deduction ignores them, as clang does, and the checks that follow
  reject the same programs as clang; now with clang's notes: "could not
  match" for an address whose specialization does not match (it was
  "candidate function template"), explicit arguments checked against
  each parameter's ABI before deduction (`h2<int>` with a
  `swift_context` `T`), at `-fclang-compat=22` "could not match" for a
  declaration or an address whose specialization fails only on its
  ABIs (clang 19's substitution failure at level 19), and a
  specialization already formed invalid noted without a reason.
- Qualified injected-class-names after `typename` or `template` (WP-1889):
  in `typename T::template X<A>` or `typename T::X` where `T` is the class
  `X` itself, the name is the constructor's ([class.qual]/2). As clang
  does, kiln now takes the template or type with a
  `-Winjected-class-name` warning, and fails the substitution in a SFINAE
  context. Since 1.3.0 kiln had taken the template silently, so libc++'s
  `std::mdspan` did not reject a layout mapping passed as its layout
  policy (libc++ 22 `mdspan/mapping.verify.cpp` failed; it passes again).
  The other libc++ 22 test that is new at 1.3.0,
  `is_within_lifetime.compile.pass.cpp`, comes from a recorded deviation:
  kiln follows P4101R1 there, where clang 22 applies the older rule (a
  default template argument may point to an escalated immediate function;
  GCC 16.1 agrees with kiln). That test stays failing; see
  docs/status/deviations.md.
- xtask test: crash fixtures no longer flake under hosted-runner load
  (WP-1888). Test and oracle binaries run with core dumps off, so a
  crashing program is reported as a signal and not as a timeout. The
  fixtures that test the timeout itself have their own directory and keep
  the short timeout.
- Structured bindings as constant template arguments (WP-1829, CWG 3135):
  a tuple-like binding of static storage duration whose `get` returns a
  prvalue names a variable, not a temporary, so `tm<b>` for
  `template<int& R>` and `tp<&b>` for `template<int* P>` are now
  accepted ([temp.arg.nontype]/6) and mangle as the variable
  (`_Z2tmIL_Z1bEEiv`, as GCC 16.1 has it); they were "reference to
  temporary object is not allowed in a template argument" errors.
  clang 22 rejects them (a documented deviation).
- FP state of implicit members and after `float_control(pop)` (WP-1808):
  an implicit or defaulted member, a defaulted comparison or an
  inheriting constructor now takes the FP state at its declaration, as
  clang does, so one declared where a file-scope `FENV_ACCESS ON` /
  `FENV_ROUND` pragma leaves the state unconstrained is no longer
  `strictfp`, and the calls inside a built body are not `strictfp` call
  sites. A `#pragma float_control(pop)` back to a state without an eval
  method now gives `source` instead of the `-ffp-eval-method=` value,
  matching clang 22 and 19.
- Emission order of imported `used` functions (WP-1832): an inline
  function with GNU `used` that an importer reaches only through the
  body of an imported function it odr-uses is now defined where clang 22
  and 19 define it (before the declaration that odr-used the function,
  in the order the module file stores that body; at its first reference
  for a callee's body, an initializer or a lambda body), in-process and
  from kind-1, kind-2 and declaration-image module files. Before, it came
  at the end of the unit. A body the module file does not carry (a
  non-inline function's) is still not read, a documented deviation.
  PCH snapshot format 103.
- An x86 `interrupt` handler whose frame parameter points to a class
  never defined now compiles (WP-1833): its `byval` is `i8`, as for
  `void *`. clang emits an opaque struct type there, which LLVM's
  verifier rejects (a documented deviation).
- A parameter qualified with an address space in a function type inside
  another declarator, a typedef or a member alias of a template (`using t
  = void (*)(T);` with `T = int __attribute__((address_space(1)))`, a
  function template's parameter or trailing return type) was accepted
  (WP-1823). It is now checked again at substitution, at the parameter,
  as clang does: an error with the instantiation note, or a substitution
  failure, so `requires { f<X>(nullptr); }` is no longer satisfied. A
  parameter declared as an array of address-space-qualified elements
  (`void f(int __attribute__((address_space(1))) a[2])`) is rejected too,
  in a template or not.
- The Swift parameter ABIs of a function template's own type and of a
  function type in its trailing return type were not checked again after
  substitution, so an empty pack leaving a `swift_error_result` without
  its `swift_context` was accepted (`g3<>(nullptr)`) and satisfied a
  requirement (WP-1839). The order, convention and nested parameter
  types are now re-checked for every substituted function type, and a
  member alias's and a variable template's errors are reported at the
  first type-specifier of the type, past a leading attribute or
  cv-qualifier, as clang does.
- A calling-convention attribute (`preserve_most`, `swiftcall`, ...) on an
  explicit specialization of a function template drew a stray "only
  applies to functions and methods" warning, a specialization naming a
  different convention than the template's was accepted, one adding or
  dropping `no_caller_saved_registers` matched the template, and an
  explicit specialization of a member function with a convention
  (`template <> void A<int>::g() {}` for `__attribute__((preserve_most))
  void g();`) was rejected as an exception-specification mismatch
  (WP-1843). The written function type attributes are now part of the
  matched type; the template's (member's) convention is adopted for the
  match and a different written one is "function declared 'X' here was
  previously declared ...", as clang.
- `-fmodule-header` with inputs that are not headers (WP-1801): a source
  is now preprocessed, to `-o` or standard output, as clang's driver does,
  instead of the "--precompile requires a module interface or partition
  unit" error; a module unit is precompiled only under `--precompile` (or
  `-fclang-compat=19`), a `.i`, `.s`, object or module file is an unused
  input, and the `-c`, `-S`, `-fsyntax-only` and `-fmodule-output` it
  leaves unused are warned about. `-E` and `-fsyntax-only` now win over
  `-c`, `-S` and `-emit-llvm` wherever they are written (`-E -c` compiled
  an object), and a lone header-unit `-x` input writes its module file to
  `-o`.
- A `static_assert` or constant initializer in a lambda body reading an
  enclosing `constexpr` variable whose value points to or refers to an
  automatic object (`constexpr const int *p = &b;` of a structured
  binding `b`, a reference to a local, a binding of a reference to a
  local) was accepted (WP-1831). Such a value is not
  constexpr-representable in the lambda body (another function parameter
  scope, P2686R5 [expr.const.init], the working draft's own
  `[&] { static_assert(a.r != nullptr); }` example), so the read is now
  "initializer of 'p' is not a constant expression", by-reference and
  by-copy captures alike; the same name in its own function, a nested
  block, an unevaluated operand and the run-time read through the capture
  are unchanged. PCH snapshot format 99.
- `__arm_new("za")` / `__arm_new("zt0")` (WP-1826): `"aarch64_new_za"` /
  `"aarch64_new_zt0"` are now attributes of the function's definition
  only, not also of its declarations and of the calls to it, as clang 22
  and 19 emit them.
- RTTI names of unit-local types (WP-1818): re-checked on both Linux
  targets. Under libc++ the `_ZTS` strings are clang 22's and 19's
  exactly (no `*` marker; new IR and two-unit run tests); under libstdc++
  kiln keeps GCC's `*` marker on purpose (WP-1651, a listed deviation),
  without which libstdc++ compares two units' same-named unit-local types
  equal.

- `constinit` added by a redeclaration after the variable was
  initialized (`int c = 1; extern constinit int c;`) is rejected with
  clang's "'constinit' specifier added after initialization of variable"
  and a note at the initializing declaration (WP-1834,
  [dcl.constinit]/1); it was accepted.
- A member function redeclared outside its class by a declaration that
  is not a definition (`void S::f();`, `D::D() __attribute__((cold));`
  after `D::D() = default;`) is rejected with clang's "out-of-line
  declaration of a member must be a definition", an error by default
  [-Wout-of-line-declaration] (WP-1835, [class.mfct]); it was accepted.
  Explicit specializations and instantiations are not affected.
- "attribute declaration must precede definition" is also given on a
  redeclaration rejected as an `inline` declaration after a non-inline
  definition, a `constexpr` mismatch, or a variable redefinition or
  conflicting redeclaration, and the `[[noreturn]]` first-declaration
  error on the first two, as in clang; neither is given any more on a
  function declared `static` after a non-static one (WP-1840).
- SVE and SME intrinsics' immediate operands are checked in sema, as
  clang's `CheckSVEBuiltinFunctionCall` / `CheckSMEBuiltinFunctionCall`
  check them (WP-1792): every ImmCheck kind of clang's `arm_sve.td` /
  `arm_sme.td` (ranges, lane and tile numbers, shift amounts, extract
  indices, rotations, multiples of 2), from a table the generator builds
  out of clang-tblgen's own (clang 22's from `-fclang-compat=20`, clang
  19's below), and a non-constant immediate is an error under
  `-fsyntax-only` too; `svext_s8(a, b, 1000)` was accepted. A
  value-dependent immediate is checked at instantiation. An explicit
  instantiation or specialization of a function template whose type has
  `__arm_streaming` or a ZA/ZT0 state attribute matches the template (it
  was "does not refer to a function template").
- Each SVE and SME intrinsic needs its own target features (WP-1793):
  clang's per-builtin feature expression (`sve2p1`, `sme2`, `i8mm`,
  `bf16`, `sme-f64f64`, ...) from `-march` or the caller's `target`
  attribute ("'f' needs target feature E"), where kiln required only `sve`
  or `sme`; and the streaming-mode rules ("builtin can only be called from
  a streaming function" / "... non-streaming function", clang 22's and
  clang 19's `VerifyRuntimeMode` evaluation) with the
  `-Wundefined-arm-za` / `-Wundefined-arm-zt0` state warnings.
- An attribute that a redeclaration adds after the definition of a
  function, variable, class or enumeration is now ignored with clang's
  "attribute declaration must precede definition" warning
  [-Wignored-attributes] and a note at the definition (WP-1807); it was
  merged silently. clang 22's exemptions apply (an attribute the
  definition already has, `alias`, C11 `_Noreturn`, the type attributes,
  and the standard-syntax `deprecated` / `nodiscard` / `maybe_unused`).
- An encoding prefix on an unevaluated string literal (`u8`, `u`, `U`,
  `L` on an attribute's string argument, a static_assert message or a
  language linkage) is only a warning before C++26 and in C, "encoding
  prefix 'u8' on an unevaluated string literal has no effect"
  [-Winvalid-unevaluated-string], as in clang (WP-1805); it was an error
  in every mode.
- The argument clause of an attribute clang does not know is skipped as
  balanced tokens in `[[...]]` and parsed as clang parses an unknown GNU
  attribute in `__attribute__` (WP-1836), so `[[clang::deprecated(u8"x")]]`
  is only "unknown attribute ignored" [-Wunknown-attributes]; it was an
  error, the arguments being checked as a known attribute's.
- `[[clang::warn_unused_result]]` is implemented (WP-1837): a discarded
  call, class or enumeration return, typedef return or temporary warns
  as for the other `warn_unused_result` spellings, with its message under
  `-fclang-compat=22`; it was ignored. In C it is an unknown attribute, as
  in clang. Every spelling now follows clang's order (callee first under
  22, type first under 19), names the return type unqualified and, under
  22, the spelling as written (`'gnu::warn_unused_result'`).
- GNU `const` or `pure` on a function returning `void` is ignored with
  clang's "'const' attribute on function returning 'void'; attribute
  ignored" warning [-Wignored-attributes] and dropped from the IR, and
  `pure` next to `const` is dropped with "'const' attribute imposes more
  restrictions; 'pure' attribute ignored" (WP-1838); the call kept
  `memory(none)` / `memory(read)`.
- The Swift parameter ABIs of an alias template's function type are
  checked again at instantiation, as in clang: a pack expansion that
  moves a `swift_indirect_result` off the front or leaves a
  `swift_error_result` without its `swift_context` is an error there
  (WP-1810); the parameter types are not checked again (`A<int>` with a
  `swift_context` on `T` was rejected).
- A calling convention written after a `*`, `&` or grouping `(`
  (`typedef int (__attribute__((swiftcall)) *fp)(int, void
  *__attribute__((swift_context)))`) applies after the function type's
  Swift parameter ABIs are checked, so they are rejected as in clang
  (WP-1827); one in the decl-specifiers reaches the function type through
  several pointers and arrays, a GNU attribute after a `&` appertains to
  the declarator, and GNU `noreturn` on a pointer to pointer or an array
  of function pointers no longer warns that it only applies to function
  types.
- The declaration image (`-fkiln-bmi-image`) carries variables with
  thread storage duration (WP-1824): `thread_local`, `__thread` and
  `_Thread_local` variables, static data members and block-scope
  statics, with their `tls_model`, where a module exporting one fell
  back to replaying its source. An importer reaches them through the
  same `_ZTW` wrappers and `_ZTH` initializers, with the same IR as a
  replay; a precompiled header's snapshot carries them too (tested).
- An importer that names an imported inline function with GNU `used`,
  an x86-64 `interrupt` handler among them, defines it `linkonce_odr` in
  `llvm.compiler.used` before the declaration that names it, with the
  `used` overloads of its name, as clang 22 and 19 do (WP-1825); it was
  only declared, without the handler's `x86_intrcc` convention.
- Programs whose importer builds a module from its declaration image
  (`-fkiln-bmi-image`) link again when an inline function names a
  constant static data member of an implicit class template
  specialization (`B<2>::v`); the member was left undefined since
  WP-1497 (WP-1817). An `alias` or `ifunc` declared in a named module is
  only declared in an importer (it was rejected there as an alias of an
  undefined symbol), and two `overloadable` `extern "C"` functions of one
  name overload instead of conflicting.
- The declaration image (`-fkiln-bmi-image`) carries every attribute
  kind (WP-1806): `code_model`, `interrupt`, `weak_import`, `alias`,
  `ifunc`, `cleanup`, `section`, `target`, `target_version`,
  `target_clones`, `annotate`, `callback`, the SME keywords, the xray,
  constructor/destructor priority and speculative-load-hardening
  attributes, Swift parameter ABIs in function types and the rest,
  where a module carrying one fell back to replaying its source. A unit
  test fails when a new attribute kind is neither carried nor exempted
  with a reason.
- Structured bindings and C++26 constexpr-referenceable objects
  (WP-1796, P2686R5, CWG 3135): a binding is referenceable and usable
  exactly as the member access or variable it denotes. A tuple-like
  binding whose `get` returns a prvalue names an object of type `Ti`
  (`const auto [a, b] = X{}; static_assert(b == 2);` with a constant
  `get` is accepted, `&b` is a constant at namespace scope and, at
  C++26, from a `constexpr` local of the same function), and a `const`
  class one is not usable. A `static` variable's initializer naming the
  automatic lifetime-extended temporary of a `constexpr` local reference
  or binding (`static constexpr const int* p = &r;`) is now rejected as
  clang 22 rejects it ("pointer to 'r' is not a constant expression").

- A block-scope `extern` function or variable declaration with an asm
  label (`extern int q(int) asm("qq");`) inside a template keeps its
  label in the instantiation; it was mangled as a local name (WP-1797).
- A program whose module exports an inline function with a local class
  now links (WP-1815): the module unit defines the local class's member
  functions (attached to the module, they are not inline) and its
  vtables even when it does not emit the function, and an importer's
  copy of the function names them, as clang does; the vtable is defined
  even when the module unit does not use it, which clang does not do.
- Value-dependent conditions in a template definition are no longer
  constant-evaluated (WP-1741).
- A constant-evaluation note about a pointer into a structured binding
  declaration's hidden variable names it as clang does (`'[x, y]'`, was
  `''`). `reflect_object` of a binding of a namespace-scope
  decomposition stays accepted, as the standard says; GCC 16.1 rejects
  it, a recorded deviation (WP-1783).
- A pointer to a bit-field member (`&C::b`) is an error, "address of
  bit-field requested", as clang (WP-1742).
- A failed constant-evaluation read of a `_BitInt` wider than 128 bits is
  noted at the operand's begin (`v[2]`: at `v`), and a wide lane's
  overflow or shift-count note names the vector type with its lanes, as
  clang (WP-1742). The alignment builtins' "requested alignment must be
  N or less" note names a typedef'd source type with its `aka`.
- In a constant expression, `p += s` and `p -= s` with a pointer `p` and a
  `_BitInt` `s` wider than 128 bits no longer compute a wrong pointer, and
  `reinterpret_cast<S>(s)` of such a `_BitInt` is not a constant
  expression (WP-1742).
- In a constant expression, a lane subscript (`v[i]`), an element
  access, `__builtin_convertvector`, `__builtin_shufflevector`, a
  reduction or an operator on a vector of `_BitInt` lanes wider than 128
  bits no longer crashes the compiler (WP-1742).
- A conditional operator with an `_Atomic` arithmetic operand (`b ? ai :
  1`) is accepted in C and C++; kiln reported "incompatible operand types
  ('int' and 'int')" (WP-1739).
- In C, the first operand of GNU `x ?: y` is converted to a value before
  it is shared, as clang does, so a volatile operand is read once
  (WP-1739).
- An alias template specialization naming an enumeration may qualify a
  name (`EA<int>::e`, [expr.prim.id.qual]/1); kiln left the expression
  erroneous without a diagnostic (WP-1739).
- Explicit instantiations and specializations that reported "not yet
  implemented" (WP-1740): `template struct S;` naming a class that is no
  member class of a class template specialization is "explicit
  instantiation of non-templated type", `template class A<int> *p;` and
  `template<> struct P q;` are diagnosed as clang does, and `template<>
  template void f();` is "expected '<' after 'template'".
- `template<class T> enum class N::E : int {}` with `N` a namespace is
  "enumeration cannot be a template" instead of being accepted
  (WP-1740).
- A `decltype(auto)` non-type template parameter deduced as an array or
  function type is adjusted to a pointer, and one-past-the-end pointer
  arguments (`A<arr + 4>`) are accepted and mangled (WP-1740).
- A constraint of a lambda nested in a generic lambda in a template, or
  of a member function of a local class there, is checked with every
  enclosing level's arguments instead of failing as dependent (WP-1740).
- Template instantiation no longer reports "not yet implemented:
  substitution into" an initialization kind (WP-1740): a parenthesized
  aggregate initialization whose aggregate has a default member
  initializer naming the function, or that has braced arguments
  (`D d(N)`, `B b({N, 1}, {N, 2})`), `T()` of a pointer to a variable
  length array, and an initialization that failed in the template's
  definition (`E e{N}` of an enumeration) are substituted, the last with
  no diagnostic beyond the definition's one.
- A conditional operator whose class-typed arms are a glvalue and a
  prvalue destroys the copy of the glvalue at the end of the
  full-expression when the result is not used to initialize an object;
  kiln never destroyed it (WP-1738).
- The "overload resolution with constraints is not supported yet" path
  is removed with the never-populated state behind it: constrained
  candidates of calls and operators were already resolved by
  [over.match.best.general]/2.6 (WP-1738; the PCH format is now 74).
- The "not yet implemented: this overloaded operator" and "this
  rewritten comparison" paths of template instantiation are removed:
  every operator call overload resolution forms is rebuilt, and a
  rewritten comparison always keeps its two written operands (WP-1738).
- A non-member `operator co_await` that cannot take the operand no longer
  turns a template coroutine's `co_await` into the "neither visible in
  the template definition nor found by argument-dependent lookup" error:
  only a viable best function is diagnosed, as in clang (WP-1738).
- A defaulted comparison first required inside a template's definition
  evaluates its body's immediate invocations (an undefined reference to
  the comparison category's consteval constructor before), and one first
  defined during a constant evaluation no longer stops kiln with an
  internal error (WP-1738).
- A member access naming a member function or typedef-name that
  redeclared a data member (or member function) as a different kind of
  entity names the valid member, as in clang, instead of reporting "this
  kind of overloaded member access is not supported yet"; other mixed
  sets are ambiguous references (WP-1738).
- The unreachable "unary operator ... is not supported yet" and
  "operator ... is not supported yet" paths of the built-in operators are
  removed (WP-1738).

- Every expression kind in a dependent signature mangles as clang's (WP-1736):
  `sizeof...` of a function parameter pack (`sZfp_`) and of a partially
  substituted pack (`sP`), a `decltype` qualifier, rewritten comparisons,
  designated initializers and expanded function parameter packs, which
  reported "cannot yet mangle expression type"; a `decltype` naming only
  non-dependent template parameters is its type (`i`), not `DT...E`; the
  kinds clang cannot mangle either give clang's errors.
- A member friend with a dependent qualifier in a local class of a
  function template is matched when the local class is instantiated
  instead of failing at its definition; `s->T::~S()` on a class object
  with a dependent `T` no longer reports "no member named '~S'";
  `T::template X<A>` naming a class template once instantiated and a
  block-scope `using T::operator+;` get clang's diagnostics instead of
  "not yet implemented"/"not supported yet" (WP-1737).
- `sizeof... T`, an attribute pack expansion `[[a...]]` and a
  mem-initializer pack expansion that initializes a member or names no
  pack are reported with clang's diagnostics instead of "not supported
  yet"; a capture pack naming a structured binding pack whose declaration
  failed no longer reports "not yet implemented" (WP-1733).
- An unnamed class template (`template<class U> struct { U u; };`) is
  reported as clang does, "cannot declare a class template with no name",
  and a class template containing one still instantiates; a concept
  declared in a class uses clang's wording (WP-1734).
- A friend function template redeclared with a different noexcept
  specifier naming its own template parameters is diagnosed once its
  class template is instantiated ([except.spec], [temp.over.link];
  WP-1734).
- Ill-formed initializations and conversions get clang's diagnostics
  instead of "not supported yet" (WP-1735): `T()` / `T(a, b)` of an
  array, reference or incomplete type, a reference initialized from
  several expressions or from `()` in a mem-initializer, a variable,
  default argument or conversion target of incomplete class type,
  static and C-style casts to an incomplete class, a variable-length
  array initialized by a braced list, an ambiguous initializer-list
  constructor, a defaulted assignment operator of a class with a
  reference or const member, and a variadic explicitly-defaulted special
  member. A nested designator into a member that is not an aggregate is
  an error (docs/status/deviations.md).

- `typeid`, a handler, or a throw of a pointer to a variable length
  array is an error, as in clang ("'typeid' of variably modified type",
  "cannot catch variably modified type"), instead of failing in code
  generation. The `type_info` of `__bf16` and the SVE types is emitted
  where it is used, as clang does, because the C++ runtime does not define
  it. A member call whose callee is wrapped in `__extension__`,
  `__builtin_choose_expr` or `_Generic`, and a lambda in an abbreviated
  function template's requires-clause or default argument, compile
  instead of failing in code generation or crashing constant evaluation.
  A template template parameter named without template arguments where
  no deduced class type may appear (`typeid(T)`, `sizeof(T)`, `T *p`) is
  an error in the template definition, as in clang (WP-1739).
- Constructs that reported "not supported yet" get clang's diagnostics
  (WP-1745): a member template named through member access, a namespace
  or template used as a value, a friend deduction guide, `__declspec`
  without `-fms-extensions`, a qualified mem-initializer naming no type,
  a using-declaration naming a destructor, `template` as an expression,
  a lambda after `delete`, a pointer to a reference member, a variably
  modified type parameter default, and the `matrix_type` and
  `riscv_rvv_vector_bits` attributes.
- A constructor defined through a typedef of its class (`T::S() {}`) and
  braced arguments to `__builtin_operator_new` / `__builtin_operator_delete`
  are accepted, as clang does (WP-1745).
- The `limit` and `clang::offset` parameters of a `__has_embed` inside
  `#if` are evaluated with the full `#if` rules (character literals,
  `uintmax_t` conversions, clang's diagnostics) instead of a reduced
  grammar that reported other forms as needing "the full evaluator"
  (WP-1744).

## [1.3.0] - 2026-11-20

A feature release with the fixes made since 1.2.1. New: an experimental
`kiln++` with the hod back end linked in (`kiln_hod/`, in-process
`-fkiln-backend=hod`); ODR metadata for linkers, `-fmason-metadata`
(section `.mason.meta`, definition hash scheme `kiln-odr-1`); GCC's C++
testsuite through `cargo xtask gxx-dg`, passing 88.8% of the decided cases
on both hosts; and the release gate as one resumable command,
`cargo xtask release-gate`, that writes a checked record. Some fixes
change behaviour on purpose, following the standard's text and, where
the text leaves room, GCC or clang 22: exposures of TU-local entities
are errors (WP-1677), a non-dependent `^^typename S::T` reflects the type
(WP-1673), iobj/xobj partial ordering accepts the calls both compilers
accept (WP-1670), and a constexpr local whose value points into itself
stays accepted at C++26 (WP-1680; docs/status/deviations.md). Nothing
in it changes what [docs/STABILITY.md](docs/STABILITY.md) promises. The
gate results are in
[docs/status/release-1.3.0.md](docs/status/release-1.3.0.md).

### Added

- `cargo xtask release-gate plan|run|control|import-x86|record|check`
  (WP-1219; docs/RELEASING.md, spec/harness.md "Release gate"): the
  release gate as one resumable command whose record page is written and
  checked by the command, not by hand. The hosted OSS rows are compared
  with the oss.md counts capped at the same run's clang control (WP-1718).

- Experimental: a `kiln++` with the hod back end linked in (WP-1703;
  `kiln_hod/`, a separate workspace, so the stock build and its lock are
  unchanged). With `-fkiln-backend=hod` it compiles objects in process
  instead of running a `hod` executable, with the same objects, answers
  and fallback (checked over `tests/codegen` and `tests/conformance`);
  `--version` names the built-in back end. `cargo xtask kiln-hod
  build|test|sweep`; CI job `kiln-hod`. `.mason.meta` keeps kiln's bytes
  through hod (WP-1561). A module without debug information now reaches
  hod without being printed and read back (WP-1704): the bridge copies
  it into hod's IR, and it is checked by comparing what it builds with
  what hod's text reader builds from the printed module (`cargo xtask
  kiln-hod bridge-eq`, `sweep --dual`); `KILN_HOD_BRIDGE=0` turns it off.
  Modules with debug information (`-g`) take the bridge too (WP-1707):
  DWARF nodes, locations and debug records are copied with the ids hod's
  reader gives them, checked the same way with and without `-g`;
  `KILN_HOD_BRIDGE_DEBUG=0` keeps them on the text path. Modules that
  need `opt` at `-O0` only for `alwaysinline` functions or call sites or
  for coroutines (most units that include standard-library headers) are
  now compiled in process too, as the external `hod` route compiles them
  (WP-1562); a module hod declines takes `opt` and `llc`, and
  `KILN_HOD_PREOPT=0` declines them all, for bisecting. The stock
  binary's behaviour is unchanged.

- `-fmason-metadata=all` names every vague definition row: its `name`
  is the symbol, so a linker can name a definition the optimizer deleted
  (WP-1586). The other levels are unchanged byte for byte.
  `scripts/mason/odr_corpus.py` builds the must-equal and must-differ
  corpus of definition hashes (`tests/mason/corpus/`, 51 cases) and its
  `manifest.tsv` for a linker's test lane
  ([spec/harness.md](spec/harness.md) "The O-13 corpus").

- `-fmason-metadata-system-unit` (WP-1619): with `-fmason-metadata`,
  marks every definition and type row of the unit's `.mason.meta` section
  as coming from a system header, for building a standard library such as
  libc++ and libc++abi from its sources, so the mortar linker treats those
  rows as it treats the library's own. Hashes, keys, the other flags and
  diagnostics are unchanged; a PCH or module file serves units with and
  without it.

- `cargo xtask gxx-dg` (WP-1443): GCC 16.1's C++ testsuite (`g++.dg`
  with `g++.dg/torture`, `c-c++-common`, `g++.old-deja`) through kiln++,
  with GCC and clang as oracles. It reads the `dg-` directives (errors and
  warnings by line, target and xfail selectors, `c++NN` standards as
  `g++-dg.exp` picks them) and sorts failures into crash, wrong-code,
  rejects-valid, accepts-invalid and wrong-line, each split by what clang
  does. The first triaged run is `docs/status/gxx-dg.md`: kiln passes
  47950 of 56766 cases. `scripts/fetch-gcc-tests.sh` fetches the C++ suites.
- `-fstack-clash-protection` and `-mstack-probe-size=` take effect
  (WP-1411): the function attributes and the `probe-stack` and
  `stack-probe-size` module flags are written as clang 19 writes them,
  and `-fno-stack-clash-protection` undoes it. The flag was accepted
  with no effect before.
- `-grecord-gcc-switches` and `-grecord-command-line`, with their
  `-gno-` forms, write the command line into the compile unit's debug
  info (WP-1411); the arguments are recorded as given.
- `-mtls-dialect=` follows clang (WP-1411): x86 accepts `gnu` and
  `gnu2`, where `gnu2` enables TLS descriptors, and AArch64 rejects the
  option.
- `-moutline` and `-mno-outline` turn LLVM's machine outliner on or off
  at `-O1` and above, as clang 19 (WP-1422); on x86-64 `-moutline` is
  clang's `-Woption-ignored` warning.
- `-ftls-model=global-dynamic|local-dynamic|initial-exec|local-exec`
  sets the TLS model of thread-local variables without a `tls_model`
  attribute (WP-1422); an invalid value is clang's error.
- `-ggnu-pubnames`, `-gno-gnu-pubnames`, `-gpubnames` and
  `-gno-pubnames` set the compile unit's name-table kind as clang 19
  does, and `-gsplit-dwarf` now gives GNU pubnames as clang's (WP-1422).
  `-gpubnames` and `-ggnu-pubnames` were rejected before.
- `-mlarge-data-threshold=` is accepted on x86-64 (WP-1419): with
  `-mcmodel=medium` or `large` it sets the `Large Data Threshold` module
  flag, with another model it is clang's
  `-Wunused-command-line-argument` warning, and on AArch64 it is an
  unsupported option, as in clang.
- `-fmerge-all-constants` and `-fno-merge-all-constants` are accepted
  (WP-1417). They have no effect: a constant `const` local array or
  class is still copied to the stack, where clang makes it a static.
- Every AArch64 NEON and fp16 builtin that kiln's `arm_neon.h` and
  `arm_fp16.h` reach now compiles, lowered as clang 19 lowers it
  (WP-1421). These are the multi-register and lane loads and stores,
  the compares, the scalar and saturating forms, the conversions, the
  SHA3/SHA512/SM crypto builtins and `vldap1`/`vstl1`. Before, 628
  builtins stopped at "not supported yet". In a census of one call per
  intrinsic, the count that compile went from 3094 to 4426 of 4884
  (docs/status/neon-builtins.md). `__has_builtin` now answers 1 for
  them.
- The GNU unwinder builtins, in C and C++ (WP-1428):
  `__builtin_dwarf_cfa`, `__builtin_unwind_init`, `__builtin_eh_return`,
  `__builtin_eh_return_data_regno` (a constant expression),
  `__builtin_dwarf_sp_column`, `__builtin_extend_pointer` and, on
  AArch64, `__builtin_sponentry`, lowered as clang lowers them. They were
  undeclared identifiers before.
- The AArch64 SME keyword attributes (WP-1426): `__arm_streaming`,
  `__arm_streaming_compatible`, `__arm_in`, `__arm_out`, `__arm_inout`,
  `__arm_preserves`, `__arm_locally_streaming`, `__arm_new` and, at
  `-fclang-compat=20` and later, `__arm_agnostic`. The type keywords are
  part of the function type (conversions, redeclarations, overriders,
  clang 20 mangling), every keyword gets clang 19's IR attributes and
  diagnostics, and SVE types may be used in a streaming function with
  `sme`. The SME intrinsics of `<arm_sme.h>` are not provided yet.
- clang's `-Wgcc-compat` warning for a GNU attribute GCC knows written
  after the declarator of a function definition (WP-1426).
- More debug info under `-g`, matching clang 19 (WP-1445): structured
  bindings are debugger variables (clang's `DIExpression` on the
  decomposed object), a variable-length array gets its variable and a
  `DISubrange` counted by `__vla_exprN`, labels get `DILabel` and
  `#dbg_label`, and the generated initializer functions
  (`__cxx_global_var_init*`, `_GLOBAL__sub_I_*`,
  `__cxx_global_array_dtor`, `__tls_init`) get artificial subprograms,
  so a breakpoint in a dynamic initializer works.
- The link-mode options `-nodefaultlibs`, `-nostartfiles`, `-nolibc`,
  `-nostdlib++`, `-static-pie`, `-static-libgcc`, `-static-libstdc++`,
  `-shared-libgcc`, `-r`, `-z <arg>`, `-u <sym>` and `-T <script>` are
  accepted by `kiln` and `kiln++` and reach the link step as in clang, in
  command-line order (WP-1453); they were unknown arguments before (found
  building ICU4C, which links a library with `-nodefaultlibs -nostdlib
  -shared`). A compile-only run warns about them as clang does.
- `-fcontracts` and `-fno-contracts`, as in GCC 16.1 (WP-1463):
  contracts (`pre`, `post`, `__cpp_contracts`) before C++26, or none at
  C++26. With libstdc++, either flag also links `libstdc++exp` (the
  default `handle_contract_violation` of libstdc++ 16) when the GCC
  installation has it, as `g++` does. GCC's
  `-fcontracts-client-check=none` and `-fcontracts-definition-check=on`
  are accepted; its other contract options are diagnosed as unsupported.
- `-fshort-enums` and `-fno-short-enums` (WP-1461): every enumeration
  without a fixed underlying type takes the smallest type that holds its
  values, as with `__attribute__((packed))`, in C and C++, as GCC and
  clang (`-fshort-enums` was rejected before).
- `-fno-kiln-debug-source-checksum` (WP-1488): under `-g`, no source
  file's MD5 checksum goes into the DWARF 5 line table, so an edit that
  moves no line (a comment in a header) leaves the objects byte-identical
  for incremental links with mortar; a debugger can then no longer detect
  a stale source. The default, `-fkiln-debug-source-checksum`, is unchanged.
- GCC's `__builtin_is_corresponding_member` and
  `__builtin_is_pointer_interconvertible_with_class` (WP-1472), so
  libstdc++'s `std::is_corresponding_member`,
  `std::is_pointer_interconvertible_with_class` and their feature-test
  macros are available, in constant expressions and at run time. They
  share one common-initial-sequence rule with `__is_layout_compatible`,
  which now also accepts members that are both `[[no_unique_address]]`
  (CWG2759, as GCC; clang answers false).
- GCC 16.1's contract flags (WP-1485): `-fcontracts-client-check=none|pre|all`
  makes the caller check the preconditions (and with `all` the
  postconditions) of each function it calls directly, and
  `-fcontracts-definition-check=off` drops a function's own checks of its
  preconditions and postconditions; `contract_assert` is unaffected
  ([spec/contracts.md](spec/contracts.md) §6.1). No ABI change.
- `-fcheck-new`, `-faligned-new[=N]`, `-fno-aligned-new` and
  `-fnew-alignment=N` (WP-1485), as clang and GCC: a null check after
  each allocation, and the threshold above which `new` calls the
  aligned allocation functions. The default output is unchanged.
- GCC's `__builtin_exit`, `__builtin__Exit` and `__builtin__exit` (WP-1485),
  calls of the library functions; never constant expressions.
- The [meta.syn] metafunctions `std::meta::is_applicable_type`,
  `is_nothrow_applicable_type` and `apply_result` (WP-1505, P1317), for a
  callable type and a tuple-like type
  ([spec/reflection-traits.md](spec/reflection-traits.md) §1.4).
- `-fmason-metadata[=min|default|detail|all]` and `-fno-mason-metadata`
  (WP-1557; kiln only, off by default): each object gets a `.mason.meta`
  section of metadata for the mortar linker's link-time checks, today a
  summary of the translation unit's ABI-affecting options. Other linkers
  drop the section; output without the flag is unchanged
  ([spec/codegen.md](spec/codegen.md) §5.5).
- `-fmason-metadata` (from level `default` up) also records the type of
  every external function and variable an object defines or uses, so the
  mortar linker can check at link time that the objects agree on it
  (WP-1564); a C-linkage entity's type is written in one canonical form
  that a C object and a C++ object share (WP-1565, after mortar's rulings:
  `int` and `unsigned int` alike, no record for C++ functions renamed by
  an asm label, C++ function types as hashes only at `default`), and
  `abi.stdlib-abi` now lists libc++'s layout-changing ABI switches
  ([spec/codegen.md](spec/codegen.md) §5.5).
- `-fmason-metadata` (from level `default` up, C++) also records a hash
  of the layout of every class an object uses (size, alignment, bases,
  members, virtual functions), so the mortar linker can report two
  objects that disagree on a class's layout (WP-1566), and of every
  enumeration an object refers to (underlying type, enumerators and
  their values; WP-1576); classes and enumerations that are local to one
  translation unit get no record. At `detail` and `all` it also records
  each class's and enumeration's members, bases, vtable slots and
  enumerators, so the linker can name the first difference (WP-1576).
  A C-linkage type no longer keeps the outer bound of an array behind a
  pointer, so `int (*)[]` and `int (*)[4]`, which C treats as
  compatible, are not reported as a mismatch (WP-1576)
  ([spec/codegen.md](spec/codegen.md) §5.5).
- `-fmason-metadata` (every level, C++) also records a row for each
  definition that may appear in several objects and that the object
  defines (inline functions and variables, template instantiations,
  explicit specializations), with a hash of its type for now, so the
  mortar linker can report an explicit specialization declared after an
  implicit instantiation elsewhere; `scripts/mason/odr_oracle.py` checks
  such objects without the linker. The unit summary names the layout
  hash scheme in `odr.layout-scheme`; `odr.scheme` now names the
  definition hash. A C-linkage function whose type has a variable array
  bound no longer gets a declaration record, and at `detail` a member of
  unnamed class type records how many entries describe it (WP-1581)
  ([spec/codegen.md](spec/codegen.md) §5.5). The hash now covers the
  whole definition: its declaration, body, constructor initializers,
  initializers of variables (as written, never their value), default
  arguments and default member initializers where they are used, and
  `new`, `delete`, `throw` and traits (WP-1582, WP-1583), lambdas
  (numbered inside the enclosing definition, with their captures, call
  operator and body), local classes and enumerations, coroutines,
  statement expressions, `asm`, label addresses, `_Generic`,
  `__builtin_choose_expr`, `offsetof` and the GNU vector and atomic
  forms (WP-1584; scheme `kiln-odr-1.d3`). A closure's call operator row
  is the closure's hash inside its enclosing definition; a function with
  a lambda in its signature, and the member functions of a local class,
  get no row. A function template specialization the definition names
  is hashed by its template, signature and template arguments rather
  than by its symbol, so two units that spell an equivalent signature
  differently agree (fmt's tests); the definition's default arguments,
  `aligned`, `noreturn`, `target`, `target_version`, `indeterminate` and
  `[[assume]]` are hashed; abi tags, visibility and
  `exclude_from_explicit_instantiation` of the entities a definition
  names are not, so a library built with its hiding macros empty
  agrees with its users; a row that uses an internal-linkage entity or
  a type local to its unit is marked `USES_INTERNAL` (WP-1585; scheme
  `kiln-odr-1.d4`). The `noexcept` of a member template of a class
  template specialization not instantiated yet, requires-expressions,
  parenthesized initializer lists in templates and the arguments of
  `target_clones` are hashed, and a function template specialization is
  hashed by its short form only beside an overload whose template
  parameters and arity could also match (WP-1626; scheme
  `kiln-odr-1.d5`). A row is marked partial only when the definition
  holds a construct not hashed yet (some dependent types, reflection):
  under 0.1 % of the rows of fmt and googletest
  ([spec/abi.md](spec/abi.md) "Definition hash (`odr_def`)"). The unit
  summary also records `abi.wchar-sign` (`signed` or `unsigned`) beside
  `abi.wchar`, and an enumeration's underlying-type record says whether
  the enumeration is scoped and whether its underlying type is fixed
  (WP-1608). With `KILN_STATS_LOG`, each job's line counts the
  definition rows, those of user code, and those of user code that name
  a standard library entity (`mason_defs`, WP-1627;
  [spec/driver.md](spec/driver.md)). With `-fmason-metadata-system-unit` the
  unit summary also says `unit.system` = `yes` (WP-1639); the summary's
  key space is open: readers skip keys they do not know unless marked
  `ENFORCE`.

### Changed

- The `.mason.meta` definition hash's scheme is `kiln-odr-1` (WP-1586):
  `kiln-odr-1.d5` frozen and renamed, no digest changed. Objects written
  under a `kiln-odr-1.dN` scheme are incomparable with `kiln-odr-1` ones;
  any later change to the hash is `kiln-odr-2`. The known misses and
  exclusions are one table, each row with the test that pins it
  ([spec/abi.md](spec/abi.md) "Misses and exclusions").

- The PCH snapshot format changed several times since 1.2.0 (new
  diagnostics, options and AST data changed the schema; the current
  version is in [spec/pch.md](spec/pch.md)). Rebuild a PCH built by
  kiln 1.2.0; until then it is not misread, as the spec describes.
- `__is_trivially_copyable`, `__is_trivial`, `__is_pod` and
  `__builtin_is_implicit_lifetime` (and with them
  `std::is_trivially_copyable`, `std::is_trivial`, `__builtin_bit_cast`,
  the atomic builtins and `std::meta::is_trivially_copyable_type`) follow
  [class.prop]/1-2 (WP-1452): a deleted or ineligible special member does
  not count, so a class whose copy and move operations are all deleted,
  or whose destructor is deleted, is not trivially copyable, and one with
  no eligible default constructor is not trivial. clang 19/22 and GCC 16.1
  each differ from the standard on some of these classes
  (docs/status/deviations.md). Argument passing, debug info and the
  `__has_trivial_*` traits are unchanged.

- Default change: at C++26 and later (`-std=c++26`, `gnu++26`, `c++2c`,
  `gnu++2c`) kiln++ links `libstdc++exp` when it links against
  libstdc++ and the GCC installation has `libstdc++exp.a` (WP-1485),
  without `-fcontracts`. The archive holds the parts of libstdc++ whose
  ABI is not frozen (the default contract-violation handler,
  `<stacktrace>`, `std::text_encoding::environment`, `<debugging>`, the
  Filesystem TS); only referenced members are linked, so other programs
  link to the same binary. g++ 16.1 does not add it at a plain
  `-std=c++26`. `-nostdlib++`, or an installation without the archive,
  leaves the link line as before; below C++26 it is still added only
  with `-f[no-]contracts`.

- `-fno-elide-constructors` is accepted before C++17 (WP-1503): the
  copies C++98 to C++14 allow to be elided are now built, and the flag
  makes them, as clang does. By default they are still elided.
- The g++.dg runner (`cargo xtask gxx-dg`) reads the severity a
  `dg-message` or `dg-regexp` pattern names, so expected errors written
  that way are no longer reported as rejects-valid, and a line-0
  `dg-error` accepts an error on any line; a case whose errors only a
  `dg-message ""`, `dg-prune-output` or `dg-excess-errors` could account
  for is counted apart as `expect-unknown` (WP-1554). The standard
  citations in docs/status/gxx-dg.md were checked against the working draft
  (WP-1556). Every CI job now has a `timeout-minutes`, so a hung runner no
  longer holds the merge queue for six hours (WP-1600).

### Fixed

- Templates (WP-1722, a regression since 1.2.1): a member function
  template deduced inside its class's definition (`using t =
  decltype(f(0));`) may look into that class from its trailing return
  type again, also a static one (`decltype(declval<U const&>().m())` with
  `U` the class): Boost.Beast's `is_fields` check passes again.
- Templates (WP-1723, a regression since 1.2.1): a class template
  instantiated from a static member function's trailing return type no
  longer gets "'this' cannot be implicitly used in a static member
  function declaration" for its own non-static members' implicit `this`
  (a non-static member template's trailing return type naming a data
  member): Boost.LexicalCast, and with it Boost.ProgramOptions and
  Boost.DateTime, build again.
- Templates (WP-1724, a regression since 1.2.1): a member function named
  only by a default template argument (`template <auto C =
  &X<T>::f>`) is instantiated where the specialization substitutes it,
  so the program links again (EnTT's `reactive_mixin`); a default of a
  namespace-scope template naming a member of a class template
  specialization links too.
- Access (WP-1726, a regression since 1.2.1): explicit template
  arguments given to a template declared in a local scope (a generic
  lambda's `operator()<I>`) no longer leave that template as the access
  context for the rest of the translation unit, which rejected a class's
  own members' bodies naming its private members and accepted private
  accesses that only the lambda had: glaze's JSON and YAML tests build
  again.
- Access (WP-1727, a regression since 1.2.1): a class or function body
  instantiated while the operand of a unary `&` naming a member of a
  dependent class (`&holder<T>::f`, `&M::constraints` in a template
  argument) is substituted no longer checks its own qualified member names
  as if they formed a pointer to member, which rejected a derived class's
  member template naming a protected member through a base class
  (`base_t::m`): eight Boost.Geometry tests build again.
- Concepts (WP-1716, a regression since 1.2.1): a nested requirement
  naming a local parameter of its requires-expression whose substitution
  fails (`requires !std::same_as<void, decltype(t.begin())>;` for a type
  without `begin`) is unsatisfied again instead of the error "an atomic
  constraint that stays value-dependent after substitution".

- Lambdas (WP-1717, a regression since 1.2.1): a by-copy capture named in
  a template argument for a non-type parameter of class type inside a
  generic lambda (`I<v[K]>`, `operator[]` returning a reference into a
  `constexpr` local) reads the captured variable again, as a scalar
  argument does, instead of "non-type template argument is not a constant
  expression".

- Precompiled headers (WP-1721, a regression since 1.2.1): a trait
  keyword the header turned into an identifier (`struct __is_arithmetic`
  in libstdc++) stays one in the file that includes the PCH, so
  `<cmath>` after a PCH of `<iostream>` compiles again instead of
  "expected expression".

- Debug information (WP-1710): `-g` stays bounded on class templates
  whose arguments repeat a specialization of the same template, whose
  names grow exponentially with the nesting (36^17 argument paths
  through 18 types in `tests/stress/linkage-pack-dag-36x17.cpp`): the
  compile grew past 4 GB and failed an 8 GiB allocation, and now takes
  0.1 s and 63 MB, as without `-g`. A specialization's debug name over
  64 KiB is the template's name alone (as clang's
  `-gsimple-template-names` spells it), its `templateParams` kept; no
  real name comes near the limit (spec/codegen.md "Bounded template
  names").

- Debug information (WP-1709): with `-g`, a virtual destructor (or any
  constructor or destructor) with a GNU asm label, whose variants are
  built into one function, gave the later variant's body a second
  subprogram; LLVM's verifier rejected the locations and the back end
  dropped the debug information of the whole module. The variants now
  share the function's subprogram.

- Reflection (WP-1673, WP-1674): a constexpr local's value is
  evaluated once, at the local's point, and the constant evaluator reuses
  it, so a nested evaluation no longer sees a class completed after that
  point (CWG3162) and hot constexpr locals cost less;
  `substitute` / `can_substitute` of a function template whose return type
  is a placeholder instantiates the definition to deduce it, as the
  standard's example says (GCC 16.1 answers false at namespace scope);
  `^^typename S::T` reflects the type denoted, not the alias
  ([spec/reflection.md](spec/reflection.md), [spec/oven.md](spec/oven.md)).

- Reflection (WP-1642, WP-1644, WP-1524): a nested constant evaluation
  (a constexpr local's initializer) no longer sees the classes its
  enclosing evaluation completed with `define_aggregate`, while the
  instantiations an evaluation triggers do (CWG3162);
  `can_substitute` may inject declarations as `substitute` does; `^^` of a
  requires-expression parameter, a splice of another function's local and
  a splice of an unnamed bit-field are diagnosed; `^^Ts...[0]::member`
  parses; annotations on parameters of block-scope or non-defining friend
  declarations and of a type that is not copy constructible are rejected;
  a call that reflects a local is never memoized, so an escaped
  reflection of a dead local is caught on every evaluation
  ([spec/reflection.md](spec/reflection.md), [spec/oven.md](spec/oven.md)).
  `g++.dg/reflect`: 476 to 481 of 565.

- `.mason.meta`: three false ODR differences reported by mortar's
  maintainers. A definition without listed attributes no longer hashes
  differently in a unit that has such an attribute anywhere (an
  `alignas` type, a `[[noreturn]]` declaration, `<stddef.h>`), so an
  inline function's row agrees across units (WP-1636); `[[noreturn]]`,
  `[[gnu::noreturn]]` and `__attribute__((noreturn))` hash alike
  (WP-1637); a lambda of an inline function named under a function type
  or template argument (libc++'s `std::visit` tables) no longer marks the
  row `USES_INTERNAL` (WP-1638). Scheme `kiln-odr-1.d5`
  ([spec/abi.md](spec/abi.md) "Definition hash (`odr_def`)").

- Codegen, found building libc++abi and libc++ 22.1.8 with kiln (WP-1620):
  the vtable of a `__cxxabiv1` `type_info` class that RTTI referred to
  before the class's own vtable was emitted is one global again (it was
  defined as `_ZTV...E.N` and left undefined under its own name); the key
  function of `__cxxabiv1::__fundamental_type_info` defines the
  fundamental `type_info` objects (`_ZTIv`, `_ZTIPKc`, ...) and their
  names, as clang and GCC do; a function first declared as a friend of a
  class under `extern template` and defined outside the class is emitted
  (libc++'s `std::__get_ostream_file`).
- Templates and concepts, found running GCC's C++ testsuite (WP-1570,
  WP-1578, WP-1598, WP-1599): a member call through `this` of the current
  instantiation naming one non-template member function with a
  non-dependent type, in a class with no dependent base, is no longer
  type-dependent ([temp.dep.expr]/5), so `auto b = this->f(1);
  b.g<int>()` needs no `template`. Before C++17 the temporaries of a
  dependent template argument no longer leak into the next
  full-expression. A concept-id with a non-type parameter whose argument
  fails is judged atom by atom ([temp.constr.atomic]/3), and a
  requires-expression under a pack expansion defers its later
  requirements ([expr.prim.req.general]/5).

- Expansion statements and related checks, found running GCC's C++
  testsuite (WP-1483): an iterating `template for` follows the working
  draft's [stmt.expand]/5.2. Its range and iterator are `constexpr`
  exactly when the loop variable is, so a non-`constexpr` loop over a
  run-time object sees the object itself. An iterator without `-` is
  rejected, and the first error in the range stops the statement. An
  array placement new into an inactive union array member makes that
  member active. A pack index of class type is converted to `size_t`. A
  name-independent `_` may follow a member function, a parameter or a
  function named `_`. A member `get` that cannot be called is reported
  once per structured binding declaration. In C23 and C++26, extra
  `va_start` arguments, discarded as before, now warn `-Wvarargs` unless
  they name the last parameter, as in GCC. `cargo xtask gxx-dg` lifts
  kiln's and clang's error limit.
- Found running GCC's C++ testsuite (WP-1485, WP-1487): a postcondition
  of a templated function with a deduced return type is accepted
  ([dcl.contract.res]/2); the ambiguous derived-to-base conversion error
  is reported at the operand and names the types as clang does; a
  braced list in a function-like macro argument is diagnosed with
  clang's notes and recovered from.
- Reflection, found running GCC's C++ testsuite (WP-1482): a range
  argument of `reflect_constant_array`, `define_static_array`,
  `substitute` and the other range-taking metafunctions is iterated once
  (`views::transform`, `views::filter`, a fixed-extent `span`, input
  ranges, a user's container), where it was read from its memory layout
  and rejected; the standard contiguous classes are still read directly.
  `extract` reads an automatic variable whose lifetime began in the
  evaluation and rejects one whose frame has returned.
  `access_context::current()` in a class template's member initializer
  names the specialization. A member of a namespace-scope anonymous union
  is a class member, and `members_of` of the namespace lists the union's
  variable. `data_member_spec` rejects built-in trait names.
- Reflection follow-ups, found running GCC's C++ testsuite (WP-1489): an
  iterated range argument whose elements are `bool` (`vector<bool>`) or
  arrays is read as its value type. Member template splices
  `x.template [:r:]` and `x.template [:r:]<args>`, splice type specifiers
  in base specifiers, mem-initializers and using-enum declarators, and a
  splice of a class or alias template used as a deduced class type are
  accepted. A splice operand of class type converts to
  `std::meta::info`. `^^C<args>::m` parses. `members_of` skips consteval
  blocks and lists the variables behind a tuple-like structured binding.
  `^^` of an anonymous union member and the union's member are the same
  reflection. `has_identifier` of a parameter depends on the evaluation
  point. `std::is_structural_type` works with GCC's `<meta>`.
- Temporary lifetime found running GCC's C++ testsuite (WP-1459): a
  reference bound through `.*` with a pointer to data member, a
  base-to-derived or dynamic cast, or a reversed subscript now extends the
  temporary; a conditional whose arms are temporaries constructs, extends
  and destroys only the chosen one, in constant evaluation too; the
  temporaries of a tuple-like structured binding's initializer live until
  its last `get` call (CWG2867, as GCC; clang 19/22 destroy them first).
  `-frange-for-ext-temps` and `-fno-range-for-ext-temps` (GCC's) turn
  P2718R0's range-for extension on before C++23 or off from it, and
  `__cpp_range_based_for` follows them.
- Template argument deduction found running GCC's C++ testsuite
  (WP-1465): of several base classes a call argument's class could
  deduce from, the most derived wins (CWG2303); an implicit deduction
  guide checks access as its class does, so a constructor naming a
  private member type deduces ([over.match.class.deduct]); taking the
  address of, or deducing `auto` from, an overload set drops members
  whose constraints are not satisfied and keeps the more constrained
  (CWG2918); `auto` deduces through array declarators (`auto (*p)[1] =
  &v;`, CWG2397); `template<class T, T n> void f(int (&)[n])` deduces
  `T` as `std::size_t` from the bound ([temp.deduct.type]/20).
- C++26 forms found running GCC's C++ testsuite (WP-1464): the C23
  one-argument `va_start(ap)` and `__STDC_VERSION_STDARG_H__` at C++26
  (P3348R4); an array placement new at the first element of an array
  object (`new (a) int[3]` for `int a[3]`, `std::allocator` storage) is a
  constant expression (P2747R2; clang 22 rejects it); an iterating
  expansion statement whose range is not a constant but whose size is
  (`std::span<int, 3>`, a member `std::array`) is evaluated at run time
  (CWG3140); a `constexpr` tuple-like structured binding whose `get` is
  not `constexpr` is an error (P2686R5; clang 22 accepts it); GNU vector
  and `_Complex` types decompose in structured bindings, as in clang.
- Crashes and hangs found running GCC's C++ testsuite (WP-1456): an
  array type larger than the target's maximum object size is "array is
  too large" (it ran out of memory); a register `asm` output of an
  aggregate that is not register sized is "impossible constraint in asm"
  (the backend aborted); `__int128` to `__bf16` on AArch64 calls libgcc
  (LLVM aborted); a recursive constraint satisfaction check stops at the
  template depth limit; `this` in a default member initializer of a
  lifetime-extended `constexpr` temporary no longer overflows the stack;
  a non-`int` `main` under `-ffreestanding` is an ordinary function; and
  a pointer to a member template in a variable template no longer
  recurses without bound.
- Reflection (WP-1462): a default-initialized `std::meta::info` is the
  null reflection and a constant (`constexpr info r;`, arrays, members,
  `new info`, through PCH and modules); metafunctions can be called
  through a pointer or reference (`views::filter(is_template)`);
  `current_class`, `current_function` and `access_context::current` in a
  consteval block and in a default member initializer used by aggregate
  initialization answer for the right scope; `size_of`, `alignment_of`,
  `bit_size_of`, `is_bit_field` and `identifier_of` accept data member
  descriptions; `reflect_constant_string` keeps the terminator of a
  `string_view` that covers it; `extract` reads through a
  constant-initialized reference. GCC's `g++.dg/reflect` goes from 306 to
  384 of 565 cases passing.
- A regression from 1.1.0, present in 1.2.0 and 1.2.1 (WP-1455): an
  out-of-line constructor whose parameter names an enclosing class of
  its own class, `O::I::R::R(I::R &&) {}`, was rejected with "use of
  undeclared identifier 'I'" and, for a protected or private `I`, an
  access error. The names after the constructor's `(` are now looked
  up in the scope of its class, as the parameters are. Found building
  LLVM's DWARFLinker.
- `-fkiln-backend=hod` (WP-1477): when hod failed on a module and kiln
  fell back to `llc`, a plain `-Werror` turned the fallback's warning
  into an error and failed the build. The warning is now
  `-Whod-fallback`, which a plain `-Werror` leaves a warning;
  `-Werror=hod-fallback` makes it an error, and `KILN_HOD_STRICT=1`
  still does.
- Debug info (WP-1445): an explicitly specialized member function's
  declaration is at the specialization, and a class with a const member
  of class type is `DIFlagNonTrivial`, as in clang.
- A precompiled header is byte-reproducible (WP-1415): two builds of
  the same header with the same kiln, command line and working
  directory, over files with the same modification times, now write the
  same bytes. Before, hash tables in the snapshot were written in a
  per-process order. spec/pch.md "Reproducibility" lists the bytes that
  depend on the invocation (the working directory, each input's path,
  size and modification time, the identity).
- A by-copy lambda capture of a variable of incomplete or abstract class
  type is clang's error (WP-1426); kiln did not diagnose it before. An
  explicit capture `[v]` of an SVE variable is checked against the
  target's features at the capture, as clang.
- C++: a typedef or alias-declaration of a class's or enumeration's name
  in its scope that names another type (`struct t1 {}; typedef long t1;`)
  is now clang's "typedef redefinition with different types", in
  namespace, class and block scope and through a using-declaration; the
  name keeps denoting the class. An alias template's conflicts are
  reported at its `using`, as clang does. After "typedef 'X' cannot be
  referenced with the 'struct' specifier" no incomplete-type errors follow
  (WP-1435).
- C: before C23, `-pedantic` now diagnoses converting, comparing or
  choosing with `?:` between pointers to differently qualified arrays
  (`int (*)[3]` and `const int (*)[3]`), as GCC does
  (`-Wc23-array-pointer-qualifiers`; clang is silent). Integer constant
  expression errors point at the first non-constant operand, without
  notes, as clang's do (WP-1435).
- `-mcmodel=` is checked against the target and the relocation model as
  clang's driver checks it (WP-1419). On AArch64, `-mcmodel=large` with
  the default PIE, `-fpic`, `-fPIE` or `-static` is now clang's error
  "invalid argument '-mcmodel=large' only allowed with '-fno-pic'";
  before, kiln wrote the large model beside PIC and the backend quietly
  emitted small-model PIC code. `kernel` and `medium` on AArch64, `tiny`
  on x86-64 and other values are "unsupported argument ... for target".
  x86-64 `large` with PIE is still accepted.
- Debug info: a function declared through a function typedef, an alias
  or `decltype` and named by a using-declaration without a definition
  in the unit (`namespace N { typedef int Fn(int); Fn h; } using N::h;`)
  no longer loses the whole module's debug information to LLVM's
  "invalid subroutine type" (WP-1434); every subprogram's type reads
  through the sugar, `noreturn` carried by the typedef included.
- Debug info: a function type with a non-default calling convention
  (`ms_abi`, `preserve_most`, `aarch64_vector_pcs`, ...) carries
  `DW_AT_calling_convention` as clang 19 writes it (WP-1434).
- A lambda's static invoker marks its call of a `noreturn` call operator
  `noreturn` and ends there, and a function whose type is `noreturn`
  ends in `unreachable` instead of `ret`, as clang emits them
  (WP-1434).
- C: a struct, union or enum tag and a typedef of the same name in one
  scope no longer conflict (WP-1427). Tags and ordinary identifiers are
  separate name spaces (C17 6.2.3), so `typedef enum {...} state;
  struct state {...};` compiles in either order, and inside the struct
  the bare name is the typedef. C++ still reports the conflict.
- C: a pointer to an array of `T` converts to a pointer to an array of
  `const T` without a diagnostic (WP-1427), as clang 19 and C23 6.7.3
  allow. Before, kiln reported "incompatible pointer types" (an error
  at the clang 22 level). The reverse conversion is now reported as
  "discards qualifiers", comparing the two pointers no longer warns, and
  `?:` gives the qualified pointer type where clang warns "pointer type
  mismatch".
- `#pragma clang attribute push(..., apply_to = function)` now reaches
  friend functions defined or declared in a class (hidden friends of
  class templates too), lambdas' call operators, explicit
  specializations of function and variable templates, deduction guides,
  and structured binding and exception variables, as in clang (WP-1417).
  A pushed `target("avx2")` over a SIMD wrapper's hidden friend
  operators no longer fails with an `always_inline` target-feature
  error, and that error now names an operator or lambda caller
  (`'operator=='`, `'operator()'`) instead of printing an empty name.
- On AArch64, a function whose `target` attribute, written or pushed,
  enables SVE (`sve`, `+sve`, `sve2`, `arch=armv9-a`, an SVE CPU) may use
  the SVE types in its signature and body without `+sve` on the command
  line (WP-1417). A lambda inside such a function gets the features of
  its own call operator, as in clang. With `sme` but no `sve` the error
  is clang's non-streaming one. A by-copy capture of an SVE variable is
  now the clang error instead of a crash in code generation.
- With `-fkiln-backend=hod`, the checks that send a unit to `llc`
  instead (function-entry instrumentation, `alwaysinline`, coroutines)
  read the function attributes themselves, not the module text
  (WP-1411): a unit that only mentions such a name in a string constant
  or a quoted symbol name is no longer sent to `llc` for it.
- An explicit instantiation of a class template no longer instantiates
  members whose trailing `requires` clause the arguments do not satisfy
  ([temp.explicit]/10; WP-1422), also in nested classes and with
  `extern template`; the emitted symbols are clang 19's. Found building
  an open-source linker.
- C: an `_Atomic` object takes the initializers, assigned values and
  arguments that its non-atomic type takes (WP-1422): `_Atomic(int *)`
  from `NULL`, `(void *)0`, `0` or `nullptr`, `_Atomic(struct S)` from a
  `struct S`, in braced, designated, nested and compound-literal
  initializers. An `_Atomic` structure is read by its value when it
  initializes, is assigned, passed, returned or cast. A C structure with
  an `_Atomic` member has clang's layout, and atomic accesses use the
  atomic type's alignment. Found building an open-source allocator.
- An explicit instantiation definition that is the first declaration of
  a namespace block (`namespace n { template class X<int>; }`, typically
  after `extern template class X<int>;`), or the first declaration after
  a namespace (`namespace a { ... } template int X<int>::f();`), defines
  each member function once (WP-1425). Each such function was built with
  two copies of its body, each with its own debug subprogram, so at `-g`
  LLVM rejected the debug info and dropped it for the whole object file.
  The second copy could not run, so the generated code was not wrong.
- A member declaration whose declarator is a parenthesized name after the
  class name is no longer read as a constructor (WP-1423): `friend S
  (::g)(int);`, `friend constexpr S (::operator""_s)(const char *,
  std::size_t);`, `S (N::g)(int);`, `S (S::*pm)(int);` and `S (f)(int);`
  in class `S` declare `::g`, the literal operator, the member pointer
  and the member function, as in clang and GCC. Before, such a friend was
  rejected, so it did not grant access. `S ((int));` is now clang's error.
- At `-g`, a class-type template argument reached only through a local
  variable's debug record (`auto w = nf<n::ic<bool, false>{}>();`) no
  longer makes the backend reject the module with "invalid type for
  undef constant" (WP-1424): the class's IR type is now defined in the
  module. libc++'s `std::not_fn<f>` tests failed to compile with `-g`.
- `offsetof` with an array index that is not a constant
  (`offsetof(struct flex, data[count])`) compiles in C and C++, as a GNU
  extension clang and GCC accept (WP-1433): the value is computed at run
  time as clang computes it, and the expression is not a constant
  expression. It was rejected as not supported before, and so was a
  constant `offsetof` whose negative index makes the offset wrap.
- Debug info: template specialization names spell value arguments as
  clang does (WP-1414): only the value's own class is named
  (`T<Out{{2}, 2.500000e-01}>`), a floating-point argument is
  `1.500000e+00` (was `1.5e0`), `INF` or `nan`, and a lambda is
  `(lambda at file:line:col)`. Diagnostics name such arguments the same
  way.
- Debug info: `defaulted: true` marks every template argument equal to
  its default, not only a trailing run, and the arguments of an explicit
  specialization (WP-1414); an alias template specialization keeps its
  typedef (`V<int>`); a member pointer keeps its pointee's typedefs; a
  generic lambda's `__invoke<T>` is artificial; a template parameter
  object used in a function template no longer adds its class to the
  retained types (WP-1414).
- A member function declared `__attribute__((used))` (or `[[gnu::used]]`)
  in a class template is instantiated and emitted when the class is
  implicitly instantiated, as clang does (WP-1430): constructors,
  destructors and conversion functions too, `linkonce_odr` and listed in
  `@llvm.compiler.used`. An ill-formed body is now diagnosed at the
  class's point of instantiation. Before, such members were emitted only
  when called, and an in-class `used` definition in a non-template class,
  or a `used` constructor or destructor, was not emitted at all.
- Explicit instantiation definitions written inside a namespace,
  `extern "C++"` or `export` block are emitted before the block's other
  declarations, as clang emits them (WP-1430); so is a static data
  member's definition at an explicit instantiation of the member or of
  its class (it was emitted with its template's namespace). The vtables
  of explicitly instantiated classes and of classes with a key function
  are emitted in one source order. Only the order of definitions in the
  output changes.
- C++: an explicit instantiation whose name is qualified must appear in a
  namespace that encloses its template ([expr.prim.id.qual]/3), as clang
  checks it: "explicit instantiation of 'E' not in a namespace enclosing
  'N'", or "must occur at global scope"; before C++23 an unqualified one
  must be in the template's namespace (WP-1448). kiln accepted these
  before. From C++23 an unqualified one is accepted (the rule was
  removed; docs/status/deviations.md).
- C++: a typedef-name declared after a using-declaration that named a
  typedef-name is checked against it: another type is "typedef
  redefinition with different types", the same type is valid (WP-1448).
- Fewer follow-on errors (WP-1448): a second `struct I` naming the same
  typedef in a block is not diagnosed again; a constructor with a
  parameter of an unknown type is not also "cannot be redeclared"; an
  invalid class gets no "must explicitly initialize the reference
  member"; a constraint that is not `bool` is not reported again as an
  invalid reference. At `-fclang-compat=22` an explicit instantiation of
  a member whose constraints are not satisfied is clang 22's "no viable
  candidate for explicit instantiation".
- A `used` member function template of a class template that is never
  defined gets `-Wundefined-inline` when the class is instantiated, as
  clang (WP-1448). A `used` destructor of a class template is accepted,
  where clang rejects it (docs/status/deviations.md).
- C: `-fsanitize=enum` (and `undefined`) no longer reports a valid C
  enumeration value as out of range (WP-1449): C has no enum range
  check, as clang; `_Bool` is still checked, and the handler data names
  C types as clang does (`'_Bool'`, `'struct S'`). At
  `-fclang-compat=19` pointer-overflow uses clang 19's C rule.
- C: a structure or union initialized from, or cast from, a `volatile`
  or `const` one (`struct S t = *vp;`, `(struct S)*vp`) is accepted; it
  was rejected as "no matching constructor" (WP-1449). An assignment to
  a `volatile` structure is a volatile copy. A same-type structure cast
  gets clang's `-Wpedantic` warning.
- Contracts, found running GCC's C++ testsuite (WP-1463): a result name
  with the name of a parameter is an error; a constructor's
  precondition or a destructor's postcondition that reaches a member
  through an implicit `this` is an error; a postcondition of a function
  template that names its result now checks each specialization's
  result. A local class's member function defined in a lambda in a
  contract or a default member initializer uses its own `this`.
- C codegen matches clang (WP-1449): padding in C constant aggregates
  and list initializations is zero at `-fclang-compat=20` and later,
  bit-field constants are one byte per element, `_Atomic` arrays are
  initialized element by element, and objects defined only by tentative
  definitions are emitted at the end of the translation unit in clang's
  order.
- `-flto -moutline` on x86-64 warns once more for the link job, as clang
  (WP-1449).
- A destructor that throws at the end of a full-expression destroys
  every object already completed in it (WP-1458; clang and GCC leak
  some): the variable, member, base, array element, delegating
  constructor's object or returned object being initialized, and
  lifetime-extended temporaries. A delete-expression whose destructor
  throws still calls `operator delete`. A class-typed statement
  expression's result is destroyed after its own temporaries.
- Wrong code found running GCC's C++ testsuite (WP-1461): a member
  partial specialization with a non-type argument of the enclosing
  template is chosen again; `T::template B` in a partial
  specialization's arguments is a non-deduced context; an explicit
  specialization of a constrained member specializes the most
  constrained matching member; `typeid` of an array of cv class drops
  the cv-qualifiers from the element too (`A3_A4_1S`); and the
  elements of a structured binding of an array are copy-initialized
  for `= e` and direct-initialized for `{e}` and `(e)`. In C, the
  constants of an enumeration whose values do not all fit in `int`
  have the enumeration's type (C23, as clang 22 and GCC).
- g++.dg wrong-code (WP-1460). A C-style or functional cast that casts
  away cv-qualification on the way to a base at a non-zero offset is a
  static_cast followed by a const_cast, so it adjusts the address
  ([expr.cast]/4.3). A base or virtual-base initializer that uses `this`
  sees the construction vtable ([class.cdtor]). `new T[n]` with a
  negative or too large bound, or with fewer elements than initializers,
  throws `std::bad_array_new_length`, or yields null with a non-throwing
  allocation function ([expr.new]/8). clang passes SIZE_MAX to the
  allocation function, and GCC throws from the nothrow form.
  `-fno-kiln-new-bound-check` restores clang's behaviour. Sized
  deallocation is on by default only from C++14, as in clang.
- C++98 to C++14: a dynamic exception specification (`throw(A, B)`,
  `throw()`) filters at run time, as clang and GCC do: a throw it does not
  allow calls the unexpected handler (`std::set_unexpected`), and what the
  handler throws propagates, becomes `std::bad_exception`, or calls
  `terminate`; kiln let the exception through or called `terminate`. In
  C++98/03 a destructor or class-specific `operator delete` declared
  without a specification may throw (it was implicitly `noexcept`)
  (WP-1457).
- Constant evaluation: a read of a union's active member through a
  glvalue of a similar type (a `const T *const &` bound directly to a
  `T *` member) was rejected as "read of member 'm' of union with active
  member 'm'"; `std::vector<T>::insert(pos, n, value)` with spare capacity
  hit it for an element owning a vector. The Oven now identifies the
  object an access reaches and the member recorded active by one rule
  ([basic.lval]/11). Found in downstream code (WP-1490).
- Reflection: `members_of` (and `static_data_members_of`) of a namespace or
  class listed partial specializations of class and variable templates,
  whose constructors' parameters had no type; they are not
  members-of-representable and are left out. Found in downstream code
  (WP-1490).
- Parser rejects-valid found running GCC's C++ testsuite (WP-1466).
  `X<T>()` and `~X<T>()` as a constructor or destructor declarator in
  class `X` are accepted with a warning in C++20 and later (CWG2237,
  `-Wtemplate-id-cdtor`, an error under `-pedantic-errors`). GNU
  attributes may be mixed with `[[...]]` in any order before a
  statement, a label or a declaration, and `[[]] [[]];` declares
  nothing silently; attribute names may be alternative tokens
  (`[[and]]`). `__extension__` silences pedantic diagnostics in the whole
  declaration or expression it starts, also in template instantiations.
  Friend declarations of constructors, destructors and conversion
  functions (`friend A::A(int);`, `friend X<T>::~X();`) befriend them.
  `p->template A<T>::f()` and `x.A<int>::template g<char>()` name a
  member. Access declarations (`A::x;` in a class) are a using-declaration
  with a deprecation warning in C++98 and an error from C++11, as clang.
  `(T())++` and `N::C::~C();` in a member function parse, and anonymous
  unions and structs in class templates are instantiated.
- Invalid code kiln accepted, found running GCC's C++ testsuite (WP-1471).
  A return-type requirement must be a type-constraint (`{ t } -> bool;` is
  an error), and one whose own arguments are dependent is checked per
  specialization. A destructor template is an error. Lambda template
  parameter lists, `[=, this]` and init-capture packs warn before C++20
  (`-Wc++20-extensions`). `typedef int Q::K;` is an error. The try block
  and handlers of a function-try-block may not redeclare a parameter.
  Too many braces around a scalar or reference initializer is an error
  under `-pedantic-errors`. A member enumeration redeclared with a
  different access is an error. A dependent placeholder constraint
  (`static inline same_as<T> auto v = 0;`) is checked in each
  specialization. A pack expansion into a non-pack alias parameter is
  caught through a template template parameter too (CWG1430). A default
  template argument that makes a later parameter's type invalid is a
  deduction failure. A member pointer to `void` or to a reference formed
  by substitution is an error. `A<n>::B` without `template` is not a
  template template argument. `= delete` after a definition is a
  redefinition, and a deduction guide may not be redeclared.
- Code clang accepts that kiln rejected, found running GCC's C++ testsuite:
  a string literal converts to `char*` with a warning, as in clang
  (`-Wwritable-strings` ExtWarn since C++11 and ranked worst in overload
  resolution; `-Wc++11-compat-deprecated-writable-strings` in C++98); a
  C-style cast between pointer-to-member types is a `reinterpret_cast`
  ([expr.reinterpret.cast]/10); `register` is a warning before C++17
  (`-Wdeprecated-register` in C++11/14); `-f[no-]access-control`; an
  inherited default constructor can be trivial (CWG2799); unnamed bit-fields
  count for standard layout (CWG1813); the injected-class-name of an
  enclosing class template names the class as a base; the implicit
  `operator=` hides a `using B::operator=`; placement `new` passes its
  default arguments to the matching placement `operator delete`; a dependent
  `T::template operator-<U>` names a function template; C++98 template
  arguments have no narrowing check. At `-fclang-compat=22`,
  `-Wno-error=<group>` keeps an extension a warning under `-pedantic-errors`
  (WP-1467).
- A call of a member function template through an object whose type is
  the current instantiation (`[o] { o.f<int>(); }`, `p->f<int>()` in a
  class template) is resolved when the member is instantiated; a const
  object calling a non-const member template was an error even in a
  member never instantiated. In a lambda capturing `*this` with a deduced
  explicit object parameter, a member of the copy is const when the
  closure object is (`decltype((x))` is `const int&`) (WP-1472).
- Constant evaluation, from GCC's C++ testsuite (WP-1468): modifying a
  const member, a const array element, a const subobject reached through
  a base or union member, a const temporary or a `new const T` object
  (by assignment, `++`, compound assignment or `memcpy`) is now an error,
  while the object's own constructor and destructor and `mutable`
  members may still write; an aggregate initializer that reads a later
  element of the same aggregate is an error; before C++26 a constexpr
  local whose value points to itself is rejected, with clang's "may
  differ on each invocation" note. Valid code that was rejected now
  compiles: a variable template specialization whose initializer binds
  a temporary, an out-of-class definition of a static constexpr array
  member with a dependent bound, `this` in a union member's default
  initializer, a block-scope `extern` declaration in a constexpr
  function (CWG2552) and a value-initialized vector in a constant
  initializer. A constexpr function calling a `<cmath>` builtin the
  constant evaluator computes exactly (`sqrt`, `fma`, the rounding,
  remainder and exponent families) is no longer reported as never
  constant before C++23.
- Variable length arrays (GNU extension), found running GCC's C++ testsuite
  (WP-1469): a lambda that captures one by reference, names a typedef or
  `decltype` of one, or takes `sizeof` of one captures its bounds as hidden
  members, so the body sees the size the array was created with;
  range-based `for` over a VLA; VLAs of class objects with constructors and
  destructors (cleanup on scope exit and on a throw); an explicit cast to a
  variably modified type evaluates its bounds, also under `sizeof`; an
  initialized VLA whose bound folds becomes a constant array
  (`-Wgnu-folding-constant`), as in clang. `-Wvla-cxx-extension` is an
  extension warning, so `-pedantic-errors` makes it an error, and
  `-Wpedantic` includes every off-by-default extension, so
  `-Wno-error=pedantic` and `#pragma GCC diagnostic ignored "-Wpedantic"`
  reach `-Wc99-extensions` and `-Wzero-length-array`.
- Accepts-invalid found running GCC's C++ testsuite (WP-1470), now
  errors as in clang: aggregate initialization that leaves a reference
  member uninitialized; before C++20, a defaulted special member whose
  type differs from the implicit one (deleted from C++20, P0641) and a
  parenthesized list initializing an array; an explicit specialization of
  a member whose requires-clause the class's arguments do not satisfy,
  and of a constrained variable template with arguments that do not
  satisfy it; an ambiguous class-scope `operator new`; a class argument
  to `...` whose copy constructor is inaccessible, a C++14 elidable copy
  or move whose constructor is unusable, and `&A::m` of a protected
  member through the base; an explicit-object member function
  redeclaring an implicit-object one ([basic.scope.scope]/4); a default
  argument added to an out-of-line member function of a class template;
  an empty pack initializing an array of unknown bound; and a class
  named as complete inside an instantiation its own definition triggers.
- A copy or move of a prvalue of the same class is elided again, as in
  clang: `B b(1)` with constructors `B(...)` and `B(const B&)` (and
  `B{1}`, `new B(1)`, `B(1)` as a cast) no longer runs the copy
  constructor at run time or in constant evaluation. A virtual base is
  now destroyed when a temporary in its own member-initializer throws
  from its destructor. `-fno-elide-constructors` (and
  `-felide-constructors`) are accepted from C++17, where they turn off
  NRVO and these copies; before C++17 the flag is rejected.
- More valid code from GCC's C++ testsuite is accepted (48 → 111 of
  133 cases of the deduction and overload rows): placement `new (0) X`
  chooses its `operator new` by overload resolution; a template-id
  naming an overload set is resolved after the call's other deductions;
  in C++98 an integral constant expression of value zero is a null
  pointer constant, with clang's warnings; aggregate class template
  argument deduction for a member template of a class template
  specialization and from nested braced lists (also with `#embed`);
  alias template deduction with a defaulted parameter (a C++20
  extension warning before C++20); inherited-constructor guides through
  a base named by a typedef; a defaulted `operator=` matching a
  `noexcept` pointer to member; and the signature of a function template
  befriended by a later redeclaration has the friend's access.
- Valid code found running GCC's C++ testsuite is accepted (WP-1495):
  a dependent qualified type before `operator` in a member declaration
  (`T::type operator+(int);`); an out-of-line constructor of a class
  template whose parameter has a member typedef type
  (`foo<C, T>::foo(bar) {}`); `class N::template X<T>` in an
  elaborated-type-specifier; a friend function with a dependent
  qualifier (`friend R T::f();`), which now befriends the member at
  instantiation; the default base access from the class-key of the
  definition (`class A; struct A : B {};` derives publicly); C++11
  keywords used as identifiers in C++98 (`struct static_assert;`); and
  C++98 access declarations with a template-id in the qualifier. GNU
  `static template` and `inline template` explicit instantiations are
  accepted with clang's `-Wstatic-inline-explicit-instantiation`
  warning, a friend constructor or destructor that matches nothing gets
  clang's candidate notes, and `-C`/`-CC` without `-E` is clang's error
  instead of feeding comments to the parser.
- Union lifetimes in constant evaluation (WP-1494): a read of a union
  member after `destroy_at` is a read of an object outside its lifetime.
  `union { int a; int b; }` members of one type at one offset are told
  apart by the glvalue that names them, for a read, a placement new and
  an explicit destruction. A single-object placement new into an element
  of an inactive array member activates the array and ends the old
  member.
- A virtual call, `typeid` or `dynamic_cast` on a sibling base of the
  object under construction or destruction is not a constant expression
  ([class.cdtor]/4-6); kiln reports it with a note naming the class
  under construction. Reads before an object's lifetime began (a later
  member in a mem-initializer, a local in its own initializer) now say
  "read of object outside its lifetime" as clang 22 does.
- C++20 and C++23 programs found running GCC's C++ testsuite: a class
  non-type template argument holding a lambda through an alias template
  or a member template's default; `decltype` of a static data member no
  longer instantiates its initializer; a qualified call to a static
  member deduces its return type before its constraints are checked; an
  init-capture (also a pack) named in its lambda's requires-clause;
  `typename T::type()` as a simple requirement; an overload set's address
  failing a requirement; a concept naming a non-static data member;
  defaulted comparisons of unions without variant members; a strong
  ordering library without `equivalent`; `static_cast` to an array type;
  libstdc++'s `source_location::current()` in a generic function; and
  `__builtin_is_pointer_interconvertible_with_class` /
  `__builtin_is_corresponding_member` on a member of an anonymous struct
  in a union.
- Valid code found running GCC's C++ testsuite is accepted (WP-1496):
  access during partial-specialization matching and template argument
  deduction is checked in the template's context (a failure in a
  function template's signature is a substitution failure), and a
  default template argument's access in its template's context;
  `V a{{}}` with an `initializer_list` constructor gives a list of one
  element; a deleted or undefined function template named in `decltype`
  is no longer diagnosed before overload resolution selects it; a friend
  function template defined in a class template is found whichever
  specialization defines it; a base class's conversion function no
  longer ranks worse than the derived class's own (`bool(y)`); an
  operator in a template finds the non-member operators visible at its
  definition, plus argument-dependent lookup at instantiation, and a
  non-dependent one keeps the function selected at definition; before
  C++17 the elidable copy or move constructor of a temporary is
  selected, access-checked and considered by `noexcept`. A `static_cast`
  to a pointer from a non-pointer (a member pointer to `void*`
  included) has clang's "cannot cast from type ... to pointer type".
- Variable length arrays and expansion statements (WP-1499): a variable
  length array of arrays of class objects (`S a[n][3]`) is constructed
  and destroyed like a flat one, also on unwinding; a lambda captures the
  bounds of every variably modified type its body uses (`typeof`, casts,
  local declarations), not only those it names; in C a bound that reads a
  `const` object makes a variable length array, folded to a constant
  array with clang's `-Wgnu-folding-constant` where one is needed. An
  iterating expansion statement checks the iterator's `++`, `!=` and `*`
  and needs both member `begin` and `end`; an uncaptured range in a lambda
  is one error, not two; and a destructuring expansion statement binds its
  range by reference instead of copying it, so a non-constant range is
  rejected as GCC does.
- Invalid code found running GCC's C++ testsuite is now rejected as
  clang 22 rejects it: `new T[n](args)` before C++20; a default
  argument on an out-of-line member of a class template or on an
  explicit specialization; a direct-initialization at C++11/14 that two
  conversion functions make ambiguous; a pack-expansion fold constraint
  before C++26 (atomic there, so neither declaration is more
  constrained); a member that would hide a using-declared base member
  whose requires-clause names different class arguments; and a class
  template's injected-class-name as its own (incomplete) base. C++11
  mode warns that a `constexpr` member function will not be implicitly
  `const` in C++14 (`-Wconstexpr-not-const`); errors in an implicit
  member or element initializer carry clang's "in implicit
  initialization of ..." note.
- A class template's friend constructor, destructor or conversion function
  whose class or type depends on the template (`friend Z<T>::Z(typename
  T::t);`) is matched again in each specialization and befriends that
  member, or is diagnosed there when no member matches. A precompiled
  header or module keeps these friends. `friend void A::f(int);` naming a
  specialization of a member function template befriends it, and is
  rejected when the template is inaccessible. A friend class declared by a
  local class names a class in the innermost enclosing block scope
  ([class.friend]/11), as GCC does. `X<T>::X(nosuch)` is one error, the
  column of a friend's qualifier points at the qualifier, and a member of a
  class template with dependent bases named through its own class in C++98
  is reported as "no member named".
- Reflection (WP-1505): `^^` of a name found through a using-declarator is
  ill-formed ([expr.reflect]/5); a splice of a template without `template`
  before it is a `-Wmissing-template-keyword` warning, as GCC, and
  `template` before a splice that is not a template is an error;
  `define_aggregate` of a type alias is rejected; only the condition of an
  `if constexpr` is constant-evaluated, not its init-statement; an
  annotation after a ref-qualifier, in a new-declarator or on an unnamed
  bit-field is rejected; `extract` of a reflected reference parameter
  reads the object it refers to. clang 22's type-trait keywords revert to
  identifiers exactly where clang 22 reverts them (`-Wkeyword-compat`), so
  `int __is_convertible;` as a member is rejected as clang and GCC do.

- The Oven tells union members apart through more pointers (WP-1508): the
  member a pointer designates survives pointer arithmetic and a store to
  memory (an array of pointers, a class member, a lambda capture, a
  reference member), and the address of a subobject reached through a
  member (`&u.s.k`, `&u.arr[1]`, `(&u.s)->k`, a base of a member) names
  that member at each union level, so reading through it while another
  member is active is rejected as clang does. Subtracting pointers to two
  members of one union is rejected ("subtracted pointers are not elements
  of the same array"); destroying an inactive union member is rejected in
  C++20 and C++23 too, as clang does; copying a union whose member was
  destroyed (or one with no active member) is accepted in C++23 as in
  C++26, and the copy keeps the member ended.
- More concepts code found running GCC's C++ testsuite is handled. A
  concept-id argument that fails to substitute makes only the atomic
  constraints that use its parameter unsatisfied, as in clang 22
  (`C<T, T&>` with `void` can still be satisfied through an operand that
  does not use the second argument), and `t.*pmf` naming a member
  function as a simple requirement is a substitution failure of that
  requirement, not an error. These are accepted now: an out-of-line
  member of a constrained partial
  specialization (matched by its whole template head), a
  using-declaration hidden by a member that corresponds only after
  substitution, a template template parameter constrained by `C<Ts...>`
  for an argument with the same constraint, an alias-sugared base pack
  expansion and a consteval default constructor in an instantiated
  array. A cast to a deleted converting constructor gets clang's
  candidate notes, and a call or operator that only finds a function
  declared after the template is clang's "neither visible in the
  template definition nor found by argument-dependent lookup" error.

- Template argument deduction and class template argument deduction
  found running GCC's C++ testsuite: with `-fclang-compat=22` the
  associated constraints are checked before the non-dependent conversions
  that may use a user-defined conversion (CWG2369, in clang 22's order); an
  aggregate deduction candidate skips a pack that is not trailing;
  copy-initialization CTAD considers only guides callable with one
  argument, and a guide whose `explicit(b)` depends on a template
  parameter is resolved per deduction; `auto(x)` with a dependent operand,
  `new char[]("abc")` (C++20) and `new char[]{"abc"}`, function parameter
  packs of function type (`T... f(int)`), constant expressions such as
  template arguments inside `sizeof` and `decltype`, dependent functional
  casts to a reference type, and, in C++11, implicit `const` on a
  constexpr member function with a trailing return type and classes with
  default member initializers not being aggregates. Candidate notes name
  an implicit copy or move assignment operator, report "packs of
  different lengths" and the value of an `explicit(b)` guide, and alias
  guides print no `<null expr>` default arguments.

- Found running GCC's C++ testsuite (WP-1503, WP-1504, WP-1511): a
  single-inheritance chain of any depth gets its vtables (the builder
  ran out of fuel past about 250 classes, and virtual-base walks stopped
  at depth 256); `static int a[10] = {a[3] = 5}` is initialized
  dynamically as clang does instead of being folded to a constant that
  dropped the store; copy-initialization with an explicit copy
  constructor reports an array structured binding at clang's location,
  naming the array type, and adds the parameter note for a function
  argument; `-fcontracts-client-check=pre|all` compiles calls with
  `_Complex` arguments or results and checks the allocation and
  deallocation functions of new- and delete-expressions, as GCC 16.1
  does.

- More invalid code is rejected, found running GCC's C++ testsuite
  (WP-1513). A friend declaration of a constructor, destructor or
  conversion function template of a class template befriends it, also
  through a precompiled header or a module, so access through it is no
  longer denied. A static data member defined out of its class template
  keeps its access, and naming a private variable or function template
  through a template-id is checked. Before C++17 a copy-initialization by
  user-defined conversion checks the copy from the temporary, so the old
  `auto_ptr` idiom is rejected as clang does. Before C++20 a defaulted
  `==` or `!=` warns that it is a C++20 extension and a defaulted
  relational operator is rejected. `new C[](8, 9)` no longer needs a
  default constructor, and an element of a parenthesized array new is
  diagnosed at the allocated type. An access declaration whose qualifier
  is a template-id is parsed in every mode, and a using-declaration that
  names a constructor is rejected before C++11. `cargo xtask libstdcxx`
  takes `--std c++11` and `--std c++14`.

- A variable, parameter or structured binding whose only use is its
  reflection (`^^v`, for example as a template argument of a template
  that only queries it or uses its type) no longer gets
  `-Wunused-variable`, `-Wunused-but-set-variable` or, at file scope,
  `-Wunneeded-internal-declaration`. Reported by a user.

- Expansion statements and variable length arrays, found running GCC's
  C++ testsuite (WP-1512). A generic lambda that odr-uses an uncaptured
  local as the range of a `template for` is rejected when it is
  instantiated (it used to fail in code generation), while reading the
  value needs no capture. A non-`constexpr` `operator++` of an iterating
  expansion statement's iterator is rejected. The destructuring form
  binds a prvalue range with `constexpr auto&&`, as the working draft
  says, instead of copying it. Initializers of C++ variable length arrays
  are diagnosed where clang 22 diagnoses them, and before C++11 a `{`
  after a declarator is the error "expected ';' at end of declaration".
  In C, `sizeof(int[m])` with a file-scope `const int m` is no longer a
  constant, and a compound literal of such a type is folded with
  `-Wgnu-folding-constant` or rejected, as clang does. `cargo xtask test
  --compiler clang++-N` compares against clang N's output when no
  `--oracle` is given.

- The linkage of a `constexpr` static data member now matches clang and
  GCC. Before C++17 the member is not inline: its out-of-class
  definition (`constexpr int A::n;`) is a strong external definition,
  emitted even when unused, so two units that both define it fail to
  link, and a unit that uses the member without that definition refers
  to the external symbol instead of defining a private copy. From C++17
  the redundant out-of-class redeclaration makes the member `weak_odr`
  and always emitted, so a C++14 unit can link against it. The
  redeclaration is no longer a redefinition in C++17 (a second one is
  accepted), `-Wdeprecated-redundant-constexpr-static-def` no longer
  fires before C++17, and `alignas` on the out-of-class definition no
  longer errors. Objects built by earlier versions have the weaker
  linkage for these members (`linkonce_odr`, dropped when unused).
  Reported from linking kiln objects with mortar.

- Reflection fixes found running GCC's C++ testsuite (WP-1515, WP-1521).
  `std::meta::extract<int>(^^b)` of a block-scope constexpr variable gives
  its constant, also through a template argument, and `extract` of a
  reflected reference local works; a local whose lifetime ended in the
  same evaluation is still rejected, also by `constant_of`. `^^B::S`
  reflects the member `S` where it is not a type, `^^N::template T` and
  `^^T::template N` reflect a template, and `template` before a name that
  is not a template is rejected. `namespace A = [: r :];`, `~typename
  [: r :]()` and `typename [: r :]<args>::member` are accepted. A dependent
  namespace alias splice is reported as not supported yet.

- Regressions found by the second full run of GCC's C++ testsuite
  (WP-1473), each a behaviour kiln had before a recent fix row:
  a by-copy capture of a reference to a function is valid again; `explicit`
  in a friend declaration and a friend destructor template are rejected;
  `&f<X>` picks the more constrained template only when the two
  specializations have the same parameter-type-lists (CWG2918), so
  `&g<int>` of `g(T)` and `g(T*)` is ambiguous; a converted constant
  expression rejects a conversion function to `double` for an `int`
  template parameter; C++98 accepts an explicit instantiation in an
  enclosing namespace, as C++11 does; an array from an empty parenthesized
  pack expansion is value-initialized; before C++17 a template-id's
  argument value does not deduce the type of `template<class T, T N>`;
  before C++17 an in-class `constexpr` static data member without an
  initializer is an error again; before C++17
  the copy that ends a copy-initialization by converting constructor
  direct-initializes, so an explicit copy constructor serves it; an
  out-of-line C++11 definition of a static `constexpr` member function
  template is not made `const`; `C::m()` inside a member of a sibling
  class `B` (both derived from `A`) is a call without an object; and
  `__is_pod` of a class with `const` members is true in C++98.
- Found running GCC's C++ testsuite (WP-1547, WP-1542, WP-1545): a
  function-local `static` initialized with label addresses in a
  constructor of a class with a virtual base no longer jumps into the
  other constructor variant's code (a crash at run time); a static union or struct whose flexible array member is
  initialized gets its elements; in C++26 the destructor of an object whose
  class has a virtual base is called again (it was skipped); a string user-defined literal calls
  `operator""X(str, len)` before the GNU character-pack template;
  passing a class through `...` whose copy would select a variadic
  constructor is an error instead of a stack overflow; and from C++11 a
  candidate that needs no string-literal-to-`char*` conversion beats
  one that does, so `s != "x"` with `s` converting to `char*` is no
  longer ambiguous.
- Member templates named after a qualifier in a class member access
  (WP-1530, [temp.names]/7, [basic.lookup.qual.general]/3):
  `p->N::f<U>()` with a type-dependent `p` and a non-dependent class `N`
  (as in `decltype(impl::create<T>()->impl::create<U>())`), and
  `this->C::f<int>()` / `this->B::f<int>()` naming an overloaded member
  template of the current instantiation or of a non-dependent base, take
  a template argument list without `template`.
- nlohmann_json 3.12.0 and range-v3 0.12.0 build again (WP-1571); they
  failed after unreleased changes made for GCC's C++ testsuite (WP-1470,
  WP-1509, WP-1513), and no released version was affected. Before C++17
  the copy of the temporary in a copy-initialization by conversion copies
  the cv-unqualified temporary, so `const T x = ...;` no longer picks a
  `const T&&` constructor template or a deleted copy constructor; a member
  access from a template pattern into a class still being instantiated is
  accepted again; a dependent `= {}` default argument after a temporary in
  the parameter's type is not converted from `void`; and a nested
  concept-id with a pack parameter under an argument that fails to
  substitute is unsatisfied instead of "an atomic constraint that stays
  value-dependent".

- Constant evaluation, found running GCC's C++ testsuite (WP-1526..WP-1529):
  P2280 (references of unknown value) applies in every mode from C++11, as a
  defect report, and `this` as the object expression of a class member
  access (`this->m.f()`, `inner.size()` in a template argument of a
  non-constexpr member function) designates an object of unknown value;
  placement new or `std::construct_at` into an inactive `T[1]` union member
  activates it; `__builtin_bit_cast` of classes with base classes or
  bit-fields is constant ([bit.cast]/2; unnamed bit-fields are padding).

- Declaration specifiers and attributes, found running GCC's C++ testsuite
  (WP-1541, WP-1536): `virtual`, `explicit` and `_Noreturn` on a structured
  binding are errors, as are function specifiers on a for-range
  declaration, an expansion statement and a condition, and `constexpr`,
  `consteval` and `constinit` in any other type-specifier-seq (spec/sema.md
  has the table). An attribute-specifier-seq before an asm-declaration is
  accepted, with each attribute ignored (CWG2262). Keywords and alternative
  tokens are attribute-tokens (`[[gnu::const]]` in C, `[[using gnu: const]]`).
  A scoped attribute after a `using` prefix is an error, and a `using`
  prefix before C++17 is an extension warning. `inline template` keeps
  clang's warning; GCC rejects it.

- Constant evaluation (WP-1574, WP-1568, WP-1575, WP-1569, WP-1567):
  `std::optional::emplace` after an earlier `emplace` or `reset` is
  constant again with libstdc++ 14 and libc++ at C++20 and C++23 (an empty
  base class was taken as destroyed when the member at its address was);
  the destructors of empty virtual bases run in constant evaluation
  ([class.dtor]/13), so a constant-destruction variable of a class with
  virtual bases is no longer guarded in code generation; an assignment
  through an inactive union member of class type whose default
  constructor is not trivial, or is deleted, is not a constant expression
  ([class.union.general]/6); a union's flexible array member initializer
  with elements of class type is accepted (GNU); and `alignof` of a
  cv-qualified array typedef carrying `__attribute__((aligned))`, or of
  an array of one, is the typedef's alignment.
- GCC compatibility (WP-1587, WP-1549, WP-1551, WP-1550): a C cast to a
  union type from the type of one of its members (GNU) is accepted again,
  as in clang (`-Wgnu-union-cast` under `-pedantic`); GCC's
  `__builtin_add_overflow_p`, `__builtin_sub_overflow_p` and
  `__builtin_mul_overflow_p` are implemented, constant when their
  operands are (integer constant expressions in C); in C++ a plain
  function pointer converts implicitly to one whose function type adds
  GNU `noreturn`, as GCC accepts, with a
  `-Wincompatible-function-pointer-types` warning (in C it is clang's
  error); an `extern inline` redefinition in a GNU dialect and an unknown
  `__builtin_` function in C get clang's wording.

- Accepts-invalid cases found running GCC's C++ testsuite (WP-1532..WP-1535):
  the inherited-constructor tie-break needs equal parameter types
  ([over.match.best.general]/2.7); deduction from a derived class whose two
  bases match with different template arguments fails
  ([temp.deduct.call]/5); a copy and a move constructor reached through
  different conversion functions stay ambiguous from C++17 although a
  conversion function yields the class ([over.ics.rank]/3.3); a class-scope
  using-declaration that conflicts with a member (a class and a typedef of
  another type, an enumerator of the class's own enumeration) is an error;
  a function template specialization named as a template argument in a
  template body is instantiated with that body ([basic.def.odr]/7); a
  discarded volatile glvalue of class type must be complete from C++11
  ([expr.context]/2); a discarded constant is not odr-used, so a lambda
  does not capture it.
- Concepts, found running GCC's C++ testsuite (WP-1531, WP-1579, WP-1553):
  a requires-expression in a generic lambda checks its requirements in
  order, so a requirement after one that fails is no longer substituted
  early into a hard error ([expr.prim.req.general]/5); a concept-id whose
  default template argument fails to substitute leaves the atoms that do
  not name that parameter to be evaluated as usual, including in a nested
  requirement ([temp.constr.atomic]/1, /3); a concept with a trailing pack
  parameter gets the same per-atom treatment, so a disjunction with a
  satisfied alternative is satisfied and the notes name the failing atom;
  and GCC's `-ftemplate-depth-N` spelling is accepted as
  `-ftemplate-depth=N`.

- Sema follow-ups from GCC's C++ testsuite (WP-1593, WP-1595, WP-1572,
  WP-1588, WP-1591): a member of a nested anonymous union with a default
  member initializer no longer deletes the enclosing union's default
  constructor and is the active member in constant evaluation, and an
  anonymous union object at namespace or block scope runs its default
  member initializers (they were skipped at run time); an array whose
  element size is not a multiple of its alignment is an error ([dcl.array]);
  `this` in a static member function's trailing return type,
  noexcept-specifier or requires-clause is an error ([expr.prim.this]/3); a
  structured binding declaration in a condition whose variable is an array
  is an error ([stmt.pre]/5) and the pre-C++26 extension warning uses clang
  22's wording; a structured binding declaration at class scope gets one
  error; a function template specialization named as a template argument
  in a lambda body inside a template is instantiated.
- GCC builtins found running GCC's C++ testsuite: `__builtin_shuffle`
  with one or two vectors and a mask that need not be constant (one
  `pshufb`/`tbl` at -O2 for 16 bytes), `__builtin_shufflevector` with
  vectors of different lengths, `__builtin_speculation_safe_value` (with
  GCC's speculation barrier: `lfence` on x86_64, `isb; dsb sy` on
  AArch64), `__builtin_assoc_barrier` (an arithmetic fence under
  `-ffast-math`), `__builtin_ffsg`, `__builtin_clrsbg`,
  `__builtin_parityg` and `__builtin_puts`. clang lacks the new names, so
  `__has_builtin` answers 0 for them, as clang does. A deprecated or
  unavailable conversion function is diagnosed where it is used
  implicitly, and so is an enumerator of an instantiated class
  template's member enumeration.

- Oven fixes found running GCC's C++ testsuite (WP-1590, WP-1594,
  WP-1573): an odr-use of a by-copy capture in a constant expression
  evaluated in the lambda body names the closure's member, so reading it or
  taking its address there is not constant ([expr.prim.lambda.capture]/11,
  [expr.const]); a delete-expression deallocates the storage when the
  destructor throws ([expr.delete]/7); after a bit-cast, an indeterminate
  `unsigned char` bit-field that shares a byte with a determinate one is
  not readable in a constant expression, also through copies ([bit.cast]/2).
- Reflection cases that kiln rejected, found running GCC's C++ testsuite
  (WP-1537): `identifier_of` and `display_string_of` of a base relationship
  and of a function parameter pack; `data_member_spec` checks its
  annotations; `type_of` of a structured binding and of an enumerator inside
  its enum-specifier; `is_final` of a class being defined; access checks
  for reflected variant members and protected members; `extract` of a
  pointer variable and of a closure converted to a function pointer;
  `access_context::current()` in inheriting constructors and trailing
  requires-clauses; a splice of a base relationship accesses the direct
  base; a constructor parameter may start with a splice; `==` and `!=` on
  `std::meta::info` operands of class type that convert to it
  ([over.built]/16); a reflection of a dependent member template-id; the
  elements of an expansion statement over a non-lvalue are xvalues
  (CWG3149); `>=` after a splice in a template argument list; defaulted
  comparisons with an explicit object parameter taken by value; default
  arguments of `extern "C"` functions in different namespaces.
- Anonymous unions and structs, enumerations with an aligned attribute,
  case values in lambdas and copy-initialization through conversion
  functions (WP-1604, WP-1606, WP-1592): an anonymous union or struct
  rejects a nested type, a member function, a static member and a
  non-public data member as clang does ([class.union.anon]/1;
  `-Wnested-anon-types` for an unnamed nested class when named), and C's
  anonymous unions are unchanged; `alignof` of an enumeration with an
  aligned attribute is that alignment (it was the underlying type's), and
  an array of one whose size is not a multiple of it is an error, also in
  a template; an odr-use of a by-copy capture in a case value or a GNU
  case range outside a template is rejected (the case value now ends its
  full-expression before it is evaluated); from C++17, `B b(u)` with
  `operator B() const` beside a better `operator B&()` initializes `b`
  through `operator B() const` as clang does instead of being ambiguous
  (a recorded deviation, docs/status/deviations.md).
- Templates and builtins, found running GCC's C++ testsuite (WP-1609,
  WP-1610, WP-1612): a call through `this` of the current instantiation
  that names an overload set or a member template (`this->f(1)`,
  `(*this).f`, an implicit `this`, `C::f`, a non-dependent base's
  `Q::f`, explicit template arguments) is resolved at definition and is
  not type-dependent when instantiation cannot choose differently, so a
  `<` after a member template of its result needs no `template`; a
  type-constraint `C<A...>` is checked as the concept-id `C<X, A...>`, so
  a pack expansion onto a non-pack parameter of the concept and a wrong
  number of arguments are rejected as clang does; `__builtin_parityg`
  rejects `bool` and enumeration operands as GCC does; a specialization
  whose template arguments hold an error is dropped instead of being
  followed by a `redefinition` error.
- Required diagnostics found running GCC's C++ testsuite: pointer-to-member
  conversions through an inaccessible or ambiguous base; `inline`,
  `virtual`, `explicit` and `_Noreturn` on parameters, qualified parameter
  declarators, misplaced `void` parameters, cv-qualifiers on conversion
  functions, specifiers on free-standing tag declarations, a template
  typedef, a typedef-name destructor (`-Wdtor-typedef`), inline variables
  before C++17 (`-Wc++17-extensions`); friend forms (explicit
  specializations, tagless friend type templates, `friend` not first,
  unqualified friend constructors, destructors and conversions); `C::C`
  naming a type (accepted under `-fpermissive`), `typename` naming a
  non-type template, ambiguous qualified injected-class-names, a type alias
  template naming a non-class in a nested-name-specifier, and a repeated
  `using E::x;` in a class; x86-64 `"+f"` asm outputs are rejected as clang
  22 does instead of crashing the backend.
- `<stddef.h>` declares `::nullptr_t` in C++11 and later; floating and
  complex floating `static const` members with an in-class initializer at
  C++98 are accepted as the GNU extension (`-Wgnu-static-float-init`); a
  by-copy capture odr-used in a local class's or enumeration's scope
  (enumerator values, bit-field widths, member array bounds) is diagnosed;
  a template argument's narrowing check no longer folds it before its
  full-expression ends; a class declared after a using-declarator naming a
  type of the same name in that scope is rejected; an rvalue reference
  bound through a conversion function yielding a derived class beside a
  better lvalue conversion function now selects the copy constructor or
  `const T&` overload as clang and GCC do (WP-1615, WP-1616, WP-1621,
  WP-1622, WP-1624, WP-1625).
- Reflection inside templates and in constant evaluation (WP-1613,
  WP-1614; found running GCC's C++ testsuite): a member splice in a
  template is substituted from the member it names, not looked up again
  by name; a namespace alias of a dependent splice-specifier; a
  block-scope opaque enumeration and its later definition are one
  enumeration per instantiation; `declval` of an abominable function
  type is not formed in a type trait; a substituted constant template
  argument is manifestly constant-evaluated; a constexpr structured
  binding whose `get` returns a prvalue, and constexpr expansion
  statements over such elements, work in templates ([dcl.struct.bind]/4);
  `reflect_constant` of a class value copies it as a template parameter
  object and throws when the copy is not template-argument-equivalent
  ([temp.arg.nontype]/4); a constexpr-unknown function reference binds an
  unspecified function ([expr.const]/10), so `reflect_function` and
  `reflect_object` of one work and calling through it is diagnosed as
  clang 22 does; `define_aggregate` runs in what `substitute` instantiates;
  `variable_of` in a nested evaluation sees the function of that
  evaluation's point (CWG3162). A nested evaluation still sees classes
  the enclosing evaluation completed with `define_aggregate`.
- More required diagnostics found running GCC's C++ testsuite: an overload
  set with no target in a `for` increment or a conditional operand;
  `static_cast` of an array of `const` to `void *` and downcasts that cast
  away qualifiers; closures have no default constructor before C++20 and
  none with captures; a member of an unspecialized class template
  specialized without its own `template <>`; class template parameters are
  not forwarding references in implicit deduction guides; an inaccessible
  move constructor in a returned local is not replaced by the copy
  constructor; access of a class template named as a template template
  argument; members of an anonymous union declared private or protected;
  later-standard features in earlier modes (`typename`-free dependent types
  before C++20, `static` and `thread_local` variables in constexpr
  functions before C++23, class templates without arguments before C++17,
  static and multi-parameter `operator[]` and static `operator()` before
  C++23); an ambiguous injected-class-name found through dependent bases
  at instantiation; a friend redeclaration of an implicit special member
  without its exception specification. A class's using-declarator of its
  own enumerator is accepted, as clang 22 does (WP-1630, WP-1631).
- `sizeof` of a variable length array type, and every other use of it, in
  an instantiated function template is rebuilt per specialization (it
  failed in code generation). A module importer no longer runs an
  initializer for an out-of-class definition of a static data member
  initialized in the class, and refers to an imported strong inline member
  as `available_externally`, as clang does. Caller-side contract checks
  now cover function template specializations that are only declared in
  the unit, including explicit specializations. Before C++20 a captureless
  closure is not trivial (no default constructor), so a local one is no
  longer copied from a constant. Deep single-inheritance chains below a
  virtual base compile faster (3.7x at depth 240). The test harness has
  `clang-unit:` (a unit compiled by clang) and `link: fail`
  (WP-1520, WP-1523, WP-1525).
- Found running GCC's C++ testsuite, siblings of fixed cases: `alignof`
  of a typedef of an aligned array; a discarded `(void)c` of a constexpr
  class object no longer captures it in a lambda; keyword `alignas(*&i)`
  reading a by-copy capture is accepted where clang 22 and GCC 16.1
  accept it; a function template specialization named as a template
  argument in a generic lambda's body is instantiated and diagnosed; a
  dependent variable placeholder's type-constraint argument list is
  checked at definition, as clang 22 does; unqualified calls of a
  static-only member set or a member template of the current
  instantiation resolve at definition (WP-1547, WP-1603, WP-1640,
  WP-1533, WP-1629, WP-1628).
- Friends, copies before C++17, template arguments and module
  attachment: a befriended member template is access-checked
  ([class.friend]/9), a templated friend naming no special member is
  diagnosed, and `&V::h<int>` is accepted as clang accepts it. The copy of
  the temporary in a copy-initialization by user-defined conversion is
  built as an elidable copy before C++17, so `-fno-elide-constructors`
  makes it. A friend special member's constexpr must match its
  declaration. Before C++17 an address template argument must be `&x` or
  an array or function name, and in C++17 it must not name a subobject. A
  trivial assignment is not constexpr in C++11 constant evaluation. An
  out-of-line member definition in a module purview is diagnosed when the
  class is attached to the global module ([basic.link]/11). `cargo xtask
  libstdcxx` names the reference compiler it ran (WP-1522, WP-1634,
  WP-1646, WP-1647, WP-1648).
- C++98 under `-pedantic-errors` reports the C++11 extensions clang 22
  reports, with clang's wording and warning groups: default member
  initializers, extra `;`, a trailing enumerator comma, `>>` closing a
  template argument list, `long long` (`-Wno-long-long` silences it),
  non-class and unelaborated friend types, default template arguments of
  function templates, variable templates, and local or unnamed types as
  template arguments. In C++98 `auto` before a type-specifier is the
  storage class. An anonymous union at class scope with a storage class,
  `mutable` on an anonymous union outside a class, a non-local lambda
  with a capture-default and `new T[]()` are rejected. Explicit object
  parameters take part in partial ordering; a lambda can be an alias
  template's argument in class template argument deduction. A friend
  function defined in a class template and used before the instantiation
  that defines it, and the destructor named by a `delete` in a template
  member, are now emitted (WP-1645, WP-1652, WP-1519, WP-1518).
- A constant expression in a lambda body that passes a by-copy captured
  constexpr object to a call (`constexpr P p{3}; [=]{ constexpr int j =
  p.get(); }`, `get(p)`, a copy) reads the captured variable, as clang 22
  and GCC 16.1 do; the capture and the closure are unchanged. A friend
  declaration of a member function checks its exception specification
  against the member's (with `-Wimplicit-exception-spec-mismatch`), and
  at C++11 a friend's constexpr member function is implicitly `const`, as
  in clang 22 (WP-1655, WP-1656).
- GCC's `__builtin_has_attribute(type-or-expression, attribute)` is
  supported. It answers from the attributes stored on the operand's
  declaration and type, with matching arguments. A dependent operand,
  and a class member access in a template definition, are errors, as in
  GCC 16.1. GCC's `__remove_reference(T)` spelling is accepted, and
  `__builtin_assoc_barrier` takes a class operand. The driver accepts
  `--start-no-unused-arguments`/`--end-no-unused-arguments` and
  `--unwindlib=`, as clang does (WP-1611, WP-1633).
- A C-linkage function whose name equals the mangled name of a C++
  function in the same unit no longer stops code generation; both names
  refer to one symbol. `__restrict` pointer parameters, and `this` in a
  `__restrict` member function, are `noalias` in the IR, as in clang 22.
  An importer of a module's `export constexpr` or `const` variable sees
  its initializer (`available_externally`), and the temporary bound to an
  exported constexpr reference is one symbol for the module and all its
  importers, so its address is the same in every unit. An in-class
  inline static data member with a dynamic initializer is initialized
  before an importer's own variables, once per program (WP-1663,
  WP-1653).
- Before C++17, an address template argument must be `&id`, or an array
  or function name, and a member-pointer argument must be `&C::m`. This
  is now also checked in template definitions and for substituted
  parameters. At C++98 a null pointer argument is rejected, and
  parentheses around an address argument give clang 22's
  `-Wc++11-extensions` warning. Before C++17 a lambda's call operator is
  not constexpr, so calling a lambda in a constant expression is
  rejected, as in clang 22 and GCC 16.1. Inside a template, a friend
  declaration of a non-dependent function template specialization that
  matches no template is now rejected at the definition. A friend
  declaring a member of an explicit specialization must not have a
  non-empty template header. Once a member has been redeclared by a
  friend, the "previous declaration" for later friends is still the
  member (WP-1657, WP-1666, WP-1665, WP-1662).
- `^^decltype(l)::operator()` reflects a closure type's call operator.
  The other names that can follow a computed scope (`decltype(e)::`,
  `T...[i]::`) also work: a conversion function, a destructor and a
  `template` template-id. An annotation written before a
  lambda-declarator (`[] [[=1]] () {}`) now appertains to the call
  operator, as the text says, instead of being rejected. `decltype` of a
  splice that designates a template parameter object is `const T`
  (WP-1660).
- GCC's `__integer_pack(N)` built-in pack (libstdc++'s
  `make_integer_sequence` without `__make_integer_seq`), mangled as GCC
  mangles it, and GCC's `__is_invocable` / `__is_nothrow_invocable`
  traits are implemented; `__has_builtin` still answers 0 for the traits,
  as clang 22, so libstdc++ keeps its library path. `__builtin_has_attribute`
  answers attributes written in a type-id operand and tells
  `visibility("internal")` from `hidden` (WP-1667).
- Partial ordering of a member function template with an implicit object
  parameter against one with an explicit object parameter (or against a
  non-member operator) now gives the implicit one a reference object
  parameter, as clang 22 does, so constraints and the reference
  tie-breaker can decide calls that kiln reported as ambiguous. A static
  member against an explicit object member no longer deduces from the
  object parameter. Before C++20 a lambda in an unevaluated operand is
  rejected, and at C++17 a lambda in a template argument is rejected, with
  clang 22's diagnostics (WP-1670, WP-1671).

- Under libstdc++ (the default) the `type_info` name of a type local to
  the translation unit (in an unnamed namespace, a local class of a
  non-inline function, and types built from them) starts with GCC's `*`,
  so two units' types of the same name no longer compare equal, a handler
  no longer catches the other unit's type and a `dynamic_cast` to it
  fails, as with GCC; with `-stdlib=libc++` nothing changes
  (`-f[no-]kiln-rtti-local-marker` forces it). Array and function types
  of such types get `internal` RTTI, and an unnamed class or enumeration
  without linkage mangles as clang's `$_N` (WP-1651). In modules: an
  instantiated static data member or variable template specialization
  with dynamic initialization is a module initializer (clang 22); a
  constexpr variable of a module interface initialized to a TU-local
  value is rejected ([basic.link]/15); imported `thread_local` variables
  keep their one definition in the module's object (WP-1669).
- `cargo xtask gxx-dg`: an input file named in a test's options is
  compiled and linked with it instead of being probed as a flag; a
  line-0 `dg-error` or `dg-message` in a `dg-do link` test is read as an
  expected link error (a failing link passes, a linking one is
  `accept-invalid`); `dg-additional-sources` honours its selector and
  `linkonly`, `$subdir` is substituted and a missing source skips the
  case; a `dg-prune-output` that can only match a warning or note line
  no longer lifts the extra-error check or leaves a rejection
  `expect-unknown` (WP-1664, WP-1617).
- An uninitialized variable in a constexpr function or constructor
  before C++20 gets clang's C++20-extension warning (an error under
  `-pedantic-errors`). In a module interface, a declaration that names
  a TU-local entity (a signature, a variable's, member's or base's
  type, a default argument, the body of an inline function) is an
  error, as in GCC; clang only warns (docs/status/deviations.md). A use
  in an inline variable's initializer gets clang's
  `-WTU-local-entity-exposure` warning (WP-1677, WP-1678, WP-1676).
- A constexpr local whose value points into itself (a `this`
  default member initializer, a reference member, an element pointing
  at another element) is accepted at C++26, as [expr.const.init]/2-4
  makes it constexpr-referenceable from itself; clang 22 and GCC 16.1
  reject it (docs/status/deviations.md). Before C++26 it is rejected
  with clang's diagnostics. `__builtin_has_attribute` answers 1 for
  `nonstring` and `nocommon` where GCC keeps them, for `nonnull` on a
  function typedef or alias, and for `format` probed with only some
  of its arguments (WP-1680, WP-1675).
- Construction vtables of a deep single-inheritance chain below a
  virtual base are computed in time proportional to what they hold: a
  300-deep chain with a virtual destructor compiles in 0.7 s instead of
  6.8 s (release), with the same IR and vtable layouts (WP-1650). C
  parameters declared `int a[static 2]` get clang's `align` and
  `dereferenceable` (or `nonnull`) attributes (WP-1668). Under libstdc++
  the `type_info` name of a closure created by a `static`, `const` or
  unnamed-namespace variable's initializer, of an unnamed class at
  namespace scope and of an incomplete class in an unnamed namespace
  starts with GCC's `*`, so two units' twins compare unequal (WP-1679).
- Before C++20, default-initializing a class whose implicit default
  constructor leaves a member uninitialized (`struct T { int a; }; T
  t;`) in a constant evaluation is rejected, as clang does; a
  constexpr constructor that leaves a member uninitialized gets clang's
  C++20-extension warning. In a module interface, a default member
  initializer, a member function's default argument, an odr-use of a
  TU-local constant (`return &K;`) and the lambda a variable of closure
  type is initialized with are checked for exposures, as GCC does
  (WP-1685, WP-1684).
- docs/status/gxx-dg.md: the third full run of GCC's C++ testsuite at
  kiln `01c4bb2f5` passes 88.8% of the decided cases on aarch64 (50256 of
  56568; second run 87.2%) and on x86-64 (50475 of 56870; 87.1%). The
  triage splits the cases that still fail into kiln bugs with a row,
  kiln bugs without one, GCC-specific expectations and wording or
  location only, and xfail repros record a bit-field glvalue conditional
  stored as a whole storage unit and a C++98 explicit instantiation
  outside an enclosing namespace that is accepted (WP-1691).
- Wrong code: assigning through a conditional whose operands are
  bit-fields (`(c ? s.p : s.q) = v`, compound assignment, `++`) stored a
  whole storage unit and overwrote the neighbouring bit-fields; it now
  stores the selected bit-field only, at run time and in constant
  evaluation. Binding an rvalue reference to a bit-field lvalue is
  rejected (WP-1692). At C++98 an explicit instantiation outside a
  namespace enclosing the template is rejected as GCC does, so is one
  whose unqualified name is found outside the template's namespace
  (which GCC accepts in C++98), and one at class scope is rejected in
  every mode (WP-1693). Before C++20 a `new`
  or `delete` in a constant evaluation is rejected with clang's note
  (WP-1689).
- Wrong code found running GCC's C++ testsuite (WP-1694, WP-1700): a
  static flexible array member initialized with non-constant elements
  was emitted without them; `__builtin_clear_padding` left a
  `std::nullptr_t` member uncleared (so libstdc++'s
  `atomic::compare_exchange_strong` of such a struct failed) and cleared
  only the first element through a pointer to a VLA; the temporaries an
  iterating expansion statement's range reference extends lived to the
  end of the enclosing block instead of the statement; a conditional
  whose operands are vector elements was an lvalue that stored to the
  wrong element (now a prvalue, as clang makes it). A conditional of
  bit-fields of a `_BitInt` wider than 128 bits is evaluated in a
  constant expression.

## [1.2.1] - 2026-11-14

A bug-fix release: one ABI fix for x86-64 code built with AVX enabled.
Nothing else differs from 1.2.0, and nothing in it changes what
[docs/STABILITY.md](docs/STABILITY.md) promises. The gate results are in
[docs/status/release-1.2.1.md](docs/status/release-1.2.1.md).

### Fixed

- **ABI fix, x86-64 (WP-1418).** x86-64 code built with AVX enabled that
  passes or returns 256-bit (512-bit with AVX-512) vectors by value
  across a boundary with code built by another compiler, or by an older
  kiln, changes its calling convention to the standard one. With `avx`
  in the translation unit's features (`-mavx`, `-mavx2`, `-march=haswell`,
  `-march=x86-64-v3`, ...), a named 256-bit vector argument or return
  value, and a struct, union or one-element array whose only member is
  such a vector, now goes in a `ymm` register as the psABI, clang and GCC
  do, not on the stack and through a hidden return pointer; with
  `avx512f` the same holds for 512-bit vectors in `zmm` registers.
  Earlier kiln releases used the no-AVX convention whatever the
  features. kiln objects built before and after this fix do not mix for
  such functions: rebuild every object that defines or calls one. As
  in clang 21 and later, at the default `-fclang-compat=22` a class with
  a base narrower than itself (`struct D : Empty { __m256 v; }`) stays
  in memory; at `-fclang-compat=19` it goes in a register, as in clang 19.
  Vectors passed through `...`, code built without AVX, `ms_abi`
  functions and AArch64 are unchanged.

## [1.2.0] - 2026-11-14

Precompiled headers that are restored instead of parsed again, the
experimental hod back end for `-O0`, and the fixes made since 1.1.0. The
gate results are in
[docs/status/release-1.2.0.md](docs/status/release-1.2.0.md).

### Added

- The PCH options are documented and supported (WP-1404; `--help`,
  "PRECOMPILED HEADERS"): `-fkiln-pch-snapshot` (the default) and
  `-fno-kiln-pch-snapshot`, `-fkiln-pch-snapshot=write` (write the
  snapshot, replay when consuming), `=load` (restore, write none),
  `=check` (restore and replay, report the first difference), and
  `-fkiln-pch-validate=mtime` (the default: an input file with the
  recorded size and time is not read, one with another time is
  compared by content) or `=content` (every input file is compared by
  content). The release gate gains `scripts/pch-release-gate.sh`.
- The PCH contract is spec/pch.md (WP-1397): the file layout, the
  snapshot section and its versioning, the identity, the search-path
  rule, the producer, the consumer's fallbacks and their warnings. Each
  snapshot format is recorded with its schema hash in
  `crates/kiln_driver/pch-snapshot-format.txt`, and a test fails, saying
  what to bump, when the schema changes without a new format version. A
  PCH from another kiln build is never misread: another snapshot format
  or schema replays with `-Wpch-snapshot-unused`, another kiln version is
  `PCH file built from a different branch`, and kiln 1.0 and 1.1 PCHs
  are `uses an older format`; rebuild the PCH.
- Experimental `-fkiln-backend=hod` (also `KILN_BACKEND=hod`; WP-1371):
  at `-O0`, an object's module is handed, before `opt`, to the hod back
  end (github.com/christopherfretz/hod, an external executable found with
  `--hod=`, `KILN_HOD`, next to `llc`, or on `PATH`), with `opt` + `llc`
  as the fallback whenever hod declines or fails. The default stays
  `llvm`. spec/driver.md "Back-end selection".
- `-fkiln-backend=hod`: coroutine translation units are handed to hod
  before `opt` too (WP-1398): a module whose only `opt` triggers are
  `alwaysinline` and coroutines goes to hod, which compiles the
  coroutines itself; when hod declines, kiln runs the coroutine passes
  and `llc` as before. `KILN_STATS_LOG` lines gain `hod_skip`, the reason
  a job was not offered to hod. spec/driver.md "Back-end selection",
  "Compile statistics log".
- `-g`: imported entities (WP-1365, reported by hod). Using-directives,
  using-declarations and namespace aliases, at namespace and block scope,
  get clang's `DW_TAG_imported_module` / `DW_TAG_imported_declaration`, so
  gdb and lldb resolve names reached through them. Nothing changes at
  `-gline-tables-only` or without `-g`.
  spec/landed/codegen/WP-1365.md.
- `cargo xtask libcxx` cross runs (WP-1391, proposed by hod):
  `KILN_LIBCXX_TARGET=<triple>` compiles and links the suite for another
  Linux target (the `target=` feature follows; a cross target links the
  static archives of its `.toolchain/libcxx-N-root` tree),
  `KILN_LIBCXX_EXEC="<words>"` runs the test binaries and `%{exec}`
  through an executor such as `qemu-x86_64`, and
  `KILN_LIBCXX_TIMEOUT_SCALE=<n>` stretches both run limits (10 under
  emulation). Unset, nothing changes. spec/libcxx.md "Cross targets".

### Changed

- Precompiled headers are fast by default (WP-1404): a PCH now carries a
  snapshot of the compiler's whole front-end state at the end of the
  header, and a consumer restores it instead of processing the header
  again. On `<bits/stdc++.h>` a `-fsyntax-only` consumer takes
  0.57 s against 1.48 s for the replay and 0.67 s for clang 19's
  PCH ([docs/status/pch.md](docs/status/pch.md)). Whenever the snapshot
  cannot be used the consumer replays the header as before, so a build
  never fails for it: silently (a `-v` note) for the cases that replay by
  design, and with one `-Wpch-snapshot-unused` warning when the
  configuration differs (another `-std`, target, `-D` list, warning
  options, a changed input file, another kiln build). A restored
  consumer does not repeat the header's warnings. A header whose state
  at its end cannot be saved (an open global module fragment, for
  example) still gets its PCH and one warning, `precompiled header
  '<pch>' cannot carry a snapshot (<why>); its consumers process the
  header again [-Wpch-snapshot-unused]`. The PCH is larger: 35.2 MB
  for `<bits/stdc++.h>` (clang 19: 26.7 MB), 8.5 MB for four
  common headers. A PCH is a trusted artifact: it is tied to the exact
  kiln binary that wrote it and to its input files (both are checked,
  and a mismatch replays or is rejected as in clang), and it is not
  meant to be shared between builds of kiln. `-fno-kiln-pch-snapshot`
  gives the 1.1 behaviour on either side. `-Wpch-snapshot-unused` only
  reports lost time, so a plain `-Werror` leaves it a warning (WP-1397,
  as clang's `DefaultWarnNoWerror`); `-Werror=pch-snapshot-unused` makes
  it an error, and a producer that cannot save its snapshot then fails
  and writes no PCH. The PCH differences from clang are in
  [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md#precompiled-headers).
  spec/pch.md.
- The PCH snapshot (WP-1403): a consumer whose include search path
  differs from the producer's (an
  extra `-I`, another order) now restores the snapshot when every header
  lookup the prefix made finds the same file, as a user or system header,
  as before; the producer records those lookups in the PCH (section
  format version 2; a version 1 snapshot is refused and replayed). A
  prefix built from absolute paths restores from any working directory,
  and `-nostdinc*` are compared through the search list they build. A
  snapshot refused for its configuration now warns once (`warning:
  precompiled header 'p.pch' was built with a different configuration
  (<why>); the header is processed again [-Wpch-snapshot-unused]`, on by
  default) and replays; `-v` names the first lookup that changed.
  spec/pch.md "Lookup-tolerant search path".
- `-g` (the default limited kind) describes a class this unit never
  required complete as a declaration, as clang does
  (`isCompleteDefinitionRequired`, WP-1389): a class used only through
  pointers, references or typedefs is completed by the debugger from the
  unit that requires it. Standard-library units shrink to clang's
  size (`<string>`: 16784 -> 15042 bytes of `.debug_info`, clang 19
  15126). A using-declaration of a function with a deduced return type
  now follows clang's emission point. Nothing changes without `-g`.
  spec/landed/codegen/WP-1389.md.

- Consteval-only values follow P4101R1 (the working draft's value-based
  model) instead of P2996R13's consteval-only types (WP-1328, first
  part). What is ill-formed is now decided by value: an immediate
  object (one holding a non-null reflection, or a pointer to an
  immediate function or into an immediate object) must be a `constexpr`
  variable, a template parameter object, or live within a constant
  expression, and an expression naming one is an immediate constant
  expression, so it escalates or is an error outside an immediate
  function context. Newly accepted: variables, members and pointers
  whose reflections are all null (`std::meta::info x;`,
  `thread_local info t;`), non-`constexpr` references bound to an
  immediate object, and run-time reads of the other members of a
  `constexpr` object whose reflections are null. Newly rejected: an
  immediate object used by an odr-use before its definition. Still
  rejected, as in 1.1.0: a run-time read of a member of an immediate
  object (`const char* n = e.name;`). Wording: "consteval-only value"
  replaces "consteval-only type". These are divergences from GCC 16.1,
  listed in [docs/status/deviations.md](docs/status/deviations.md).
- P4101R1, second part (WP-1361). Newly accepted (each rejected by
  1.1.0 and GCC 16.1): run-time `std::meta::info` objects, which hold
  the null reflection and are emitted as a pointer-sized null, passed
  and returned like `std::nullptr_t`; non-`consteval` functions with
  `info` in their type (parameters, return types, members, virtual
  functions, lambdas, templates, function and member pointers);
  `constexpr` pointers to immediate functions (`constexpr auto b =
  plus1;`, arrays and aggregates of them, `constexpr auto e = g();`)
  and calls through them in constant expressions (P3603); run-time
  `__builtin_bit_cast` from or to `info`, now trivially copyable,
  trivial and POD as a scalar type (a constant-evaluated one is "not a
  constant expression"). Newly rejected: a consteval override of a
  non-consteval virtual function in a class with an `info` member
  ([class.virtual]/18 as the working draft words it; a system header
  is exempt, so libstdc++ 16.1 `<meta>` still compiles). Changed:
  `__cpp_consteval` is 202606L with reflection enabled (202211L with
  `-fno-reflection`); immediate-escalation notes name the callee
  unqualified and say "evaluates the address of an immediate function
  'operator()'" for a lambda call operator, as clang 19/22 do; a
  default argument holding a consteval-only value is reported at the
  call that uses it.

### Removed

- The experimental PCH declaration image (WP-1397). Its option,
  `-fkiln-pch-image[=write|check|load]`, is accepted and ignored with
  `warning: '-fkiln-pch-image' is ignored: the precompiled header
  snapshot replaced it [-Woption-ignored]`; `-fno-kiln-pch-image` is
  accepted silently. The snapshot covers everything the image carried,
  and the PCH is the same with or without the option. The module-file
  declaration image (`-fkiln-bmi-image`) is unchanged, and module files
  are byte-identical. `cargo xtask test --pch-diff` keeps `replay` and
  `snapshot`; `load` and `check` are gone with the image.

### Fixed

- PCH snapshot hardening (WP-1396, WP-1399). A prefix file that changes
  after the consumer's check (a same-size edit with its time put back,
  or any edit under `-fno-validate-pch`) ends the compile with `fatal
  error: file '<f>' has been modified since the precompiled header
  '<pch>' was built` instead of compiling against text it did not check;
  a damaged lookup log or section is refused and replayed, never
  misread. A pending `#pragma weak` (also `weak alias = target`) and a
  pending `#pragma redefine_extname` cross the PCH as they do with
  `-include`; a header that ends inside `#pragma clang attribute push`
  is an error, as in clang.
- Sema correctness (WP-1405). In C++ a variable length array parameter
  keeps its type, so an out-of-line member definition, a redeclaration or
  a friend whose VLA parameter differs no longer matches (as in clang). A
  lambda cannot implicitly capture a member of a block-scope anonymous
  union. A class or variable template partial specialization whose
  non-type parameter's type differs from the primary's no longer matches
  by converting the value ([temp.deduct.type]/20, as GCC). Calling a
  deprecated member template by name warns for the template, then the
  specialization. Clang's wording for partial pack lengths with explicit
  arguments, a deduced template that does not match, constraint notes on
  `sizeof...` and folds, `-Wpointer-arith` on pointers to a zero-size VLA,
  and the `aka` of a pointer to a typedef'd VLA.
- Sema correctness (WP-1359, WP-1334, WP-1386). `Z<TTs<Us>...>` in a
  member template of a class template with a template template parameter
  pack `TTs` is expanded (deduction, explicit arguments, return types,
  requires-clauses; mangled with clang's `_SUBSTPACK_`) instead of being
  rejected as not implemented. In a template, `this->sm` naming a
  `constexpr` static data member of dependent type that holds a
  reflection is a consteval-only leaf in each instantiation. A variably
  modified type is no template argument, so a coroutine with a variably
  modified parameter is an error as in clang. In C, two pointers to
  variable length arrays of compatible element types can be subtracted.
- Coroutine fixes (WP-1374, found by the hod project). A coroutine's own
  parameter objects are kept out of the coroutine frame, as clang 22
  does, so a parameter the ramp destroys after the frame is freed (a
  `trivial_abi` class, or one whose copy into the frame throws) no
  longer reads freed memory. The return object is kept out of the frame
  when a postcondition names it, so `post(r: ...)` checks it after the
  coroutine completed in its ramp. A variable length array in a
  coroutine is the error "variable length arrays in a coroutine are not
  supported" (as clang), and a call to `__builtin_alloca`, `alloca` or
  their aligned forms in a coroutine is an error too (stricter than
  clang, whose sema accepts it); both used to stop the build in LLVM
  ("Coroutines cannot handle non static allocas yet"). One LLVM bug
  remains: when `get_return_object()` throws, LLVM can keep the ramp's
  exception slots in the freed frame
  ([docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md)).
- A class-type (or array) non-type template argument with a pointer or
  reference subobject designating a string literal, `__func__`, a
  temporary or a `typeid` result (`X<T{"x"}>`, at any depth: members,
  bases, array elements, a union's active member) is now rejected, as
  [temp.arg.nontype]/6 requires and clang and GCC do (WP-1362; a
  `fixed_string` holding a `char[N]` copy stays valid). A top-level
  `__func__` argument is named "predefined '__func__' variable", as
  clang does.
- An error in a non-type template argument of a template-id formed by an
  instantiation is reported at the argument, not at a stale location of
  another template-id or at the template name (WP-1362).
- A reference template argument naming a bit-field is reported as
  "reference cannot bind to bit-field in converted constant expression"
  (clang's wording) instead of as binding a temporary (WP-1362).

## [1.1.0] - 2026-11-12

C mode, and the fixes made since 1.0.2. The gate results are in
[docs/status/release-1.1.0.md](docs/status/release-1.1.0.md).

### C mode

kiln compiles C. Its C behaviour is tested against clang (`-x c`),
diagnostic wording included. What is not done is in
[docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#c-mode), and the
measurements are in [docs/status/c-mode.md](docs/status/c-mode.md).

- Drivers and dialects: `kiln` compiles `.c` (and `.h`, `.i`) as C, and
  `-x c` selects C under either driver. `kiln++` compiles a `.c` file as
  C++, with clang++'s deprecation warning. `-std=c89` (`c90`,
  `iso9899:1990`), `c99`, `c11`, `c17` (`c18`), `c23` (`c2x`), each
  with its `gnu` dialect; the default is `gnu17`, as in clang 19. C has
  its own lexing (no C++-only punctuators, user-defined literals or
  alternative tokens; no `//` comments in strict C89), C keywords
  (`_Imaginary` and `_Decimal32/64/128` are rejected as unsupported
  types, as in clang), each standard's identifier characters for UCNs,
  clang's C predefined macros and feature-test answers
  (`__has_c_attribute`; `__has_cpp_attribute` is C++ only), and C
  linkage (unmangled names, `fn.var` static locals),
  so C and C++ objects link together (WP-1275, WP-1290).
- Assembly inputs (WP-1331): `.s` (`-x assembler`) is assembled and
  `.S` / `.sx` (`-x assembler-with-cpp`) preprocessed by kiln in
  assembler-with-cpp mode (`__ASSEMBLER__`, `#` comments and `'` passed
  through) and then assembled, by `-c` or in a link, mixed with C and
  C++ inputs, with `-Wa,` / `-Xassembler` passed to the assembler (the
  LLVM installation's `clang++ -c -x assembler`). `-E`, `-S` and
  `-fsyntax-only` on a `.S` write the preprocessed text; on a `.s` they
  warn `'assembler' input unused`, as clang. Before, `-c x.s` warned
  `'linker' input unused`, wrote no object and exited 0.
- `<tgmath.h>` compiles in every C dialect (WP-1331): before C23 a
  function declared with only `...` is accepted when it is
  `overloadable`, as clang does; before, the header was rejected below
  `-std=c23`, including at the default `gnu17`.
- `-std=` takes every spelling clang 19 and 22 take, with their meaning
  (WP-1331): `-ansi` (`c89` for C, `c++98` for C++; the last of it and
  `-std=` wins), C94 `iso9899:199409` (`__STDC_VERSION__ 199409L`,
  digraphs), `c2y` / `gnu2y` (`202400L`), `iso9899:199x` and
  `iso9899:201x`; `iso9899:2024` is invalid at `-fclang-compat=19`, as in
  clang 19. Strict `-std=c89` no longer lexes digraphs. OpenCL and HLSL
  standards are "not allowed with 'C'" (or `'C++'`), and an invalid value
  lists the language's standards in notes, as clang does.
- `#embed` in C (WP-1331): no warning at `-std=c23`/`gnu23` and later
  (only `-Wpre-c23-compat`), and "#embed is a C23 extension" before C23,
  as clang 19/22; before, every C dialect warned "#embed is a Clang
  extension", which broke `-Werror` C23 builds.
- C23 `_BitInt` literal suffixes `wb` / `uwb` (WP-1331, any of clang's
  spellings: `WB`, `uWB`, `wbU`, ...): the type is the narrowest
  `_BitInt` holding the value (`0wb` is `_BitInt(2)`, `0uwb`
  `unsigned _BitInt(1)`), in code and in `#if`, with clang's
  "a C23 extension" warning before C23 and `-Wpre-c23-compat` from C23
  on; C++ still rejects them, as clang. Before, they were "invalid
  suffix". A literal over 128 bits is still "too large" on x86-64, where
  clang accepts it.
- C semantics, each where C differs from C++ (WP-1276, WP-1285 to
  WP-1287, WP-1301, WP-1311):
  - conversions: `void *` to and from any object pointer, integer and
    pointer conversions and incompatible pointer types diagnosed as
    clang does (`-Wint-conversion` is an error by default;
    `-Wincompatible-pointer-types` is an error by default, as in clang
    22, and a warning at `-fclang-compat=19`), pointer casts to narrower
    integers, `void *` and function-pointer arithmetic (GNU), C pointer
    comparisons and `?:` composite types, no C++ narrowing in brace
    initializers or case labels;
  - types: character literals are `int`, string literals are not
    `const`, `!`, comparisons and `&&`/`||` yield `int`, an enumeration
    with no negative enumerator promotes to `unsigned int`, `_Generic`
    matches by C compatibility;
  - declarations: the separate tag name space, tentative definitions
    (`-fcommon` and the `common` attribute), C99 and GNU89 `inline`
    (`-fgnu89-inline`), functions without a prototype, K&R
    definitions, implicit `int` and implicit function declarations
    (warnings in C89, errors from C99, as in clang), implicit
    declarations of the C library functions clang predeclares, no
    overloading (`__attribute__((overloadable))` keeps it), and
    prototype-scope tags and enumerators;
  - arrays and objects: variable length arrays, `[static N]`,
    `[restrict]` and `[*]` in array parameters, compound literals as
    lvalues (and file-scope compound literals as static objects),
    initialization of a `char` array by a string that exactly fills
    it, flexible array members initialized in static objects (GNU);
  - static initializers fold what clang folds (address constants
    through casts and offsets, pointer and label-address differences,
    IEEE floating results), and a C static is never initialized at run
    time: a non-constant initializer is clang's error;
  - statements: `goto` past an initialized declaration (but not into
    the scope of a VLA), and falling off the end of a non-void function
    without a trap (C only makes using the value undefined);
  - C99, C11 and C23 constructs: `_Bool`, `_Complex`, designated
    initializers, `_Static_assert`, `_Noreturn`, `_Alignas`, `_Alignof`,
    `_Thread_local`, `_Atomic`, anonymous structs and unions, C23
    `bool`/`true`/`false`, `nullptr`, `constexpr` objects, `auto`,
    `typeof`/`typeof_unqual`, `enum E : T`, `static_assert` without a
    message, `= {}`, `[[attributes]]` and `_BitInt(N)`.
- GNU C extensions, as clang implements them: statement expressions,
  `typeof`, `?:` with an omitted middle operand, case ranges and
  designator ranges, labels as values and computed `goto`, local labels
  (`__label__`), old-style `field:` designators, zero-length arrays,
  `transparent_union`, `cleanup`, `packed`, `__int128`, vector types,
  inline assembly and `__builtin_types_compatible_p` (WP-1276,
  WP-1287, WP-1288, WP-1290).
- What builds: five C projects with their own build systems pass their
  tests with kiln as the C compiler: zlib 1.3.2, sqlite 3.54.1, Lua
  5.4.8, c-ares 1.34.9 and zstd 1.5.7 (2026-11-11). GCC 16.1's
  `gcc.c-torture` suites at `-std=gnu23`, `-O0` and `-O2`: 6912 of 7572
  cases pass, against 6887 for clang 19 (2026-11-11, kiln at
  `-fclang-compat=19`). Every case kiln fails, clang 19 fails too
  (WP-1277, WP-1282, WP-1295, WP-1301, WP-1304).
- Diagnostics for the C-only rules are clang 22's, or clang 19's with
  `-fclang-compat=19`, including `-Wstrict-prototypes`,
  `-Wdeprecated-non-prototype`, `-Wincompatible-library-redeclaration`,
  `-Warray-parameter`, `-Wsizeof-array-argument` and
  `-Wsizeof-array-decay` (the last two in C++ too), C types printed as C
  (`struct S`, `_Bool`) (WP-1312).
- Linking: `kiln` links as `clang` does, so a C program does not pull
  in libstdc++ or libm; link C++ objects with `kiln++` (WP-1282).
- Precompiled C headers: `-x c-header` (or a `.h` input under `kiln`)
  writes a PCH that a C compile takes with `-include-pch` or by the
  `-include` probe (WP-1275, WP-1303).
- C mode matches clang on excess initializers (a -Wexcess-initializers
  warning, an error only under `-pedantic-errors`; the extra clauses are
  dropped and never evaluated), rejects `extern "C"` ("expected
  identifier or '('"), treats every `?:` as a non-lvalue, and rejects
  unary `+`, `-` and `~` of an array or function (WP-1318).

### Added

- Precompiled headers: kiln's PCH works as in 1.0 (`-x c++-header`,
  `-emit-pch`, `-include-pch`, and now `-x c-header`): by default the
  consumer validates the PCH and then replays the header's tokens, so a
  PCH build costs what the same build with `-include` costs. 1.1.0 adds
  an experimental serialized form behind
  `-fkiln-pch-image[=write|check|load]` (`-fno-kiln-pch-image`), off by
  default: the producer also writes the header's preprocessor state
  and, when it can carry every declaration of the header, a
  declaration image; a `load` consumer then builds the declarations
  from the image instead of parsing the header, and `check` builds both
  ways and reports the first difference. A header holding anything the
  image cannot carry, and every C header, gets no image, and its
  consumers replay it; the two paths are never mixed. The image
  does not yet carry most standard library headers, so real prefixes
  still replay ([docs/status/pch.md](docs/status/pch.md)). It is
  planned as the default for 1.2.0 (spec/driver.md "The declaration
  image", WP-1280, WP-1036, WP-1293).
- GNU extensions in C++ too: local labels (`__label__`), old-style
  `field:` designators (`-Wgnu-designator`),
  `__builtin_extract_return_addr`, `__builtin_frob_return_addr` and
  `__builtin___clear_cache` (WP-1288).
- Driver: `-rpath <dir>` is passed to the link; `-Wa,<args>` and
  `-Xassembler <arg>` take the integrated assembler's options clang
  takes (`--noexecstack` and others) (WP-1288).
- Profile-guided build of kiln itself: `cargo xtask build --pgo` and
  `cargo xtask dist --pgo` build instrumented, train on a fixed corpus
  from `bench/`, merge with `llvm-profdata` (rustc's LLVM major, found in
  kiln's LLVM or rustup's `llvm-tools`) and rebuild with the profile. That
  is about three times the build time, for a faster compiler with the
  same memory use. `cargo xtask pgo-profile --out <file>` is the pre-step
  alone, for a packaging system that runs its own `cargo build --release`
  with `-Cprofile-use=<file>`. `cargo xtask pgo-train` is the training
  alone. Nothing changes without the switch (README.md "Profile-guided
  build", WP-1308).
- `-fkiln-memory-report` has two exact rows for constant evaluation:
  `oven.heap-peak`, the most evaluation memory held at once in the
  translation unit (memory freed again during an evaluation still counts),
  and `oven.static-peak`. The total counts them instead of the capacity of
  the Oven's memory buffers, now shown as `oven.mem.capacity` outside the
  total (spec/oven.md "Memory report: the Oven's peaks", WP-1315).

### Changed

- The experimental declaration image of module files and PCHs
  (`-fkiln-bmi-image`, `-fkiln-pch-image`; both off by default) carries
  more of C++: locations in other files and in macro expansions,
  declaration attributes, linkage specifications, inline and unnamed
  namespaces, class, alias and variable templates with their
  specializations and member templates, friend templates, explicit
  instantiations, constraints, deduction guides, expressions in types
  and template arguments, block-scope `extern` declarations and
  `static_assert` in templates. Whatever it does not carry falls back
  to the 1.0 replay. `-fkiln-bmi-image-census` prints, per reason, what
  a unit's image would refuse (WP-1070, WP-1278, WP-1279, WP-1283,
  WP-1284, WP-1292, WP-1293, WP-1300, WP-1307, WP-1310).

### Fixed

- Compile time: a large braced initializer whose elements name
  constant variables (a generated table such as `{names[3], 4}, ...`
  over a `constexpr` array, or `{v, v, ...}`) was quadratic in its
  element count. Since 1.0.0 that included elements that subscript a
  constant array or access a member of one, so an 8,000-element constant
  table took 1.2 s instead of 0.5 s, and 16,000 elements 2.8 s; it is
  linear again (1.0 s at 16,000) (WP-1335).
- C++: unary `+` of a function (`+fn`) is a function pointer instead of
  an error, and `-`/`~` of an array or function report the decayed
  pointer type, as clang (WP-1318).
- Crash: a GNU binary conditional whose condition is a logical,
  relational or `!` expression (`(a || b) ?: c`) no longer stops code
  generation with the internal error "phi operand %N is defined", in C
  and C++ (WP-1288).
- Crash: an `asm` statement with a `_Complex` operand no longer crashes
  the backend; the operand is passed indirectly, as clang does
  (WP-1288).
- `__builtin_constant_p` of a value that is not a constant expression
  is left to the optimizer (`llvm.is.constant`), as in clang, so it is
  1 once inlining makes the argument constant. It was always 0
  (WP-1288).
- AArch64: the NEON table builtins `vqtbl2` to `vqtbl4`, `vqtbx1` to
  `vqtbx4`, `vtbl3`, `vtbl4` and `vtbx1` to `vtbx4` were "not supported
  in code generation". They are lowered as clang 19 lowers them
  (WP-1288).
- Parsing: a later declarator of a group may begin with GNU attributes
  (`void f(void), __attribute__((noreturn)) g(void);`), in a member
  declaration too (WP-1301). A compound literal after `sizeof` or
  `alignof` (`sizeof (struct P){ 0 }`) is the operand expression, not a
  type-id followed by a stray brace (WP-1287).
- Accepts-invalid:
  - a `static` namespace-scope variable after a declaration with
    external linkage (a block-scope `extern` included), and a
    non-`static` one after a `static` first declaration, are errors
    ([dcl.stc]/4); a block-scope `extern` declaration now redeclares a
    later declaration in any block of its namespace ([basic.link]/6)
    (WP-1293);
  - a function whose first declaration lacks `[[noreturn]]` while a
    later one has it is an error ([dcl.attr.noreturn]/1) (WP-1311);
  - a `reinterpret_cast`, or a C-style cast performing one, in the
    initializer of a `constexpr` or `constinit` variable is an error
    ([expr.const]/5.15). The same initializer of a variable that is
    neither is still static data, with no guard or dynamic
    initialization (WP-1295);
  - pointer arithmetic on a pointer to a variable length array (the GNU
    extension in C++) is not a constant expression (WP-1287).
- Module and PCH outputs: `--precompile -o /dev/null` or
  `-emit-pch -o /dev/null` (any path that exists and is not a regular
  file) is written in place, as clang does. Run as root, kiln replaced
  `/dev/null` with a regular file (WP-1281).
- Diagnostics: an error in an `#if` expression has clang's caret
  ranges (WP-1290).
- **Source-compatibility note: previously accepted code is now
  rejected.** A run-time use of an expansion-statement (`template for`)
  element variable whose type holds a reflection was accepted:
  `template for (constexpr auto e : plan) std::puts(e.name);` with
  `struct E { std::meta::info r; const char* name; }`. It is ill-formed
  (P2996R13 [expr.const]: `e` is an expression of consteval-only type
  outside a constant-evaluated context), as the same use of a plain
  `constexpr E e = plan[0];` already was, and is now rejected with the
  same "consteval-only expressions are only allowed in a
  constant-evaluated context" in every expansion form, for `constexpr
  const auto&` and `constexpr auto&&` element variables, for a
  structured-binding element declaration, and in function templates
  (also for a `constexpr T x` of a function template instantiated with
  such a class, and a binding of a `constexpr` structured binding of
  one). GCC 16.1 rejects it too. Write `constexpr const char* name =
  e.name; std::puts(name);` instead: the read is then inside a constexpr
  variable's initializer. No file in kiln's own tree relied on the old
  acceptance (WP-1330).

### Testing and tooling (not user-visible)

- The OSS harness builds C with kiln (`kiln-cc`, a per-project `c` key,
  per-language TU counts), with the five C rows above (WP-1277).
  `cargo xtask gcc-torture` runs GCC 16.1's `gcc.c-torture` suites
  through kiln, GCC and clang and writes the generated tables of
  docs/status/c-mode.md (WP-1277, WP-1282). The `tests/c/` corpus has
  clang 19 `-x c` as its oracle (WP-1275).
- PCH: `cargo xtask test` and `cargo xtask libcxx` take
  `--pch-diff load|replay|check`, a differential of the image and the
  replay path, run over a corpus slice in `cargo xtask ci` (WP-1036).
  `cargo xtask bench bench/pch` measures kiln's PCH against clang 19's
  and writes docs/status/pch.md, judging kiln's accounted memory rather
  than peak RSS (WP-1294, WP-1303).
- `cargo xtask bench` judges every kiln entry of `frontend`, `oven`,
  `overload` and `reflect` on kiln's accounted memory (the
  `-fkiln-memory-report` total, deterministic) at 2%, from one extra
  unmeasured compile, beside the RSS gate; the table shows `acct MB` and
  a `judged` cell per row. A kiln version bump no longer makes kiln's own
  baselines advisory: since the 1.0.0 bump every main-group comparison
  had been advisory and no gate judged anything. The four groups are
  re-recorded. A reflection shape's `--gcc-toolchain=` path is recorded
  as a placeholder (spec/bench.md, WP-1315).

## [1.0.2] - 2026-11-11

A bug-fix release: fixes found while building downstream code,
each with a test. Nothing in it changes what
[docs/STABILITY.md](docs/STABILITY.md) promises.

### Fixed

- Wrong code: the default member initializers of a member class template
  were lost when a member declaration of the enclosing class instantiated
  the template before the end of that class
  (`struct Outer { template <class T> struct F { bool set{false}; T v; }; F<bool> f; };`).
  A constructor or aggregate initialization then left those members
  uninitialized. The initializer is now found at its first use; a use
  before it is parsed is the error it is for a non-template class
  ([class.mem.general]/8).
- Linking against libstdc++: a member explicitly defaulted on its first
  declaration is defined where it is used even when its class has an
  explicit instantiation declaration, as clang does (`linkonce_odr`).
  `std::filesystem` directory iteration failed to link without it.
- Debug info: a constexpr member of an explicitly instantiated class
  template (`template class std::vector<T>;`) could be emitted twice
  when an earlier member called it, and with `-g` LLVM then dropped the
  translation unit's debug info.
- Debug info: an explicit instantiation definition of a class template
  (`template struct V<int>;`) completes the specialization's type and
  keeps it in the compile unit's retained types, as clang does. The type
  was described only if a member that referred to it was emitted.
- Linking: a function called in the operand of `typeid` whose result is a
  glvalue of polymorphic class type is odr-used, so it is defined or
  instantiated: `typeid(*p)` with a class template's member `operator*`
  no longer leaves an undefined symbol at link time. A non-polymorphic
  operand still names nothing it odr-uses ([expr.typeid]/3).
- Overload resolution: a comparison whose regular and reversed rewritten
  candidates tie (`a < b` through `operator<=>(const Base&)`, or `a == b`
  through the pre-C++20 `operator==(const Base&)`) is a substitution
  failure in a requires-expression, a concept or a SFINAE'd signature, as
  in clang and GCC; outside SFINAE it is still accepted with
  `-Wambiguous-reversed-operator`.
- Lookup: the namespace-name of a using-directive or of a namespace alias
  is looked up among namespaces only ([basic.lookup.udir]). A class of
  the same name in a nearer scope no longer gives "expected namespace
  name".
- Constraints: a function's constraints are checked from the function, so
  a hidden friend's trailing requires-clause may name a private member of
  the befriending class. Such a friend was dropped from overload
  resolution, and a deduced-return template was chosen in its place.
- Placement new: a placement parameter of type reference to a base class
  accepts an argument of a derived class
  (`operator new(std::size_t, Allocator &)` with a derived allocator).
- Abstract classes: binding a reference to an abstract class from an
  object of an unrelated class through a converting constructor is
  ill-formed ([class.abstract]/3), and a substitution failure where one
  is allowed. GoogleTest's printer selection relied on it.
- Parsing: in `X<args>::name`, `name` is classified in the explicit
  specialization the template-id names when every argument is a decimal
  literal or `sizeof(name)`. Nested `BOOST_SCOPE_EXIT` blocks need it.
- Warnings:
  - a comparison or conditional expression with an unnamed enumeration
    warns under `-Wdeprecated-anon-enum-enum-conversion`, as in clang, so
    `-Wno-deprecated-anon-enum-enum-conversion` silences it;
  - `-Wcomment` no longer reports a multi-line `//` comment when the
    spliced-in line is itself a comment.

## [1.0.1] - 2026-11-11

A bug-fix release. Nothing in it changes what [docs/STABILITY.md](docs/STABILITY.md)
promises.

### Fixed

- Reflection mangling (the ABI note in
  [docs/STABILITY.md](docs/STABILITY.md#covered-stable-within-a-major-version)): two
  different dependent reflected template-ids could get the same symbol
  (`^^AT<T>` and `^^TT<T*>`, and `^^F<T,int>` and `^^F<T,long>` in gcc
  mode). They no longer collide (WP-1250). GCC 16.1's spelling is used
  wherever it cannot collide. Three of its spellings lose information,
  and GCC itself fails to assemble the resulting overload pairs: a
  dependent function template-id without its arguments, a qualified
  variable template-id without its qualifier, and an alias template-id
  whose aliased type does not pin down the alias's arguments. kiln keeps
  its own spelling in those three cases, so the names differ from GCC's
  there; spec/abi.md §8.14 lists them. A reflected alias whose parameter
  appears only in a non-deduced context now gets GCC's `ta` form when
  its argument is dependent, so it links with g++-16 (WP-1260). A value-dependent reflection is no longer evaluated before
  instantiation (WP-1260).
- A consteval-only reference in a template is checked again in each
  instantiation. A dependent initializer such as
  `constexpr bool b = m == members[0];` in a `template for` body is no
  longer rejected (WP-1263, P3032R2).
- A failed `consteval` block in a lambda body leaves the lambda valid
  (WP-1257). A failed `static_assert` in a lambda body no longer stops
  evaluation, so a second refusal is reported too (WP-1247).
- Diagnostics:
  - When a reflection query triggers an instantiation error, the
    diagnostic names the consteval call frames (WP-1248). Those frames
    still show under copy elision and nesting, and spec names keep their
    template arguments (WP-1258).
  - Text the program provides (an uncaught exception's `what()`,
    attribute messages, reflected names) is escaped as clang escapes it
    (WP-1259).
  - An uncaught exception's `what()` keeps the characters read before a
    fault, and its length cap does not split a UTF-8 sequence (WP-1251).
  - An invalid escape in an unevaluated string (a `static_assert`
    message, `[[deprecated]]`, `[[nodiscard]]`, `extern "…"`, asm) no
    longer causes follow-on errors. The construct is dropped, as in
    clang 22. Escape diagnostics quote the whole escape and use clang 22's
    source ranges (WP-1268).
  - Column widths and the escaping of unprintable characters use LLVM
    22's Unicode 15.1 tables, so an unassigned code point is shown as
    `<U+XXXX>` as clang shows it (WP-1268).
- Invalid UTF-8 in an ordinary string or character literal is the
  `-Winvalid-source-encoding` warning, not an error, as in clang
  (WP-1267).
- Under `-fkiln-source-path-style=gcc`, header paths are spelled as
  g++-16 spells them (the `-I` directory exactly as given, `"../x.h"`
  relative to the includer). This is the name that `source_location` and
  `std::meta::source_location_of` report (WP-1262).
- C++20 modules: the experimental declaration image
  (`-fkiln-bmi-image`) now covers imports, partitions and
  using-declarations (WP-1068).

### Testing and tooling (not user-visible)

- The harness can run GCC 16.1 as the compiler under test
  (`--compiler $KILN_GCC`). It runs clean over the reflection and
  contracts tests (WP-1249), and every harness command maps its flags for
  GCC (WP-1261, WP-1266). Test compiles pass `-fno-crash-diagnostics`
  (WP-1264).
- The clang oracle retries once when a compile dies on a signal
  (WP-1254).
- The sweep workflow's `oss` job fails when it produces no report
  (WP-1241). It runs as one `oss-matrix` job over the `sweep = true` rows
  of `projects.toml` (WP-1265).
- `release.yml` uploads the dist archives from the workspace, and can be
  started by hand as a dry run (WP-1253).
- Spec notes with pinning tests: `type_of(reflect_constant(^^int))`
  (WP-1255), and `reflect_constant` of a function reflection's value,
  which instantiates nothing (WP-1252).

## [1.0.0] - 2026-11-10

The first release. This section summarises what kiln 1.0 does. Measured
results are in [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md), and the
limits are in [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md).

### Language

- C++98 through C++23, and C++26 as in the Croydon DIS draft: 260 of the
  261 rows of [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md) are
  done. The one partial row is P2786 (trivial relocatability), which was
  removed from C++26 at Kona and is not in the DIS.
- C++26 static reflection (P2996, `<meta>`), on by default at
  `-std=c++26` (`-fno-reflection` turns it off). The `<meta>` header is
  GCC 16.1's libstdc++ one, selected with `--gcc-toolchain=`; libstdc++
  14 and libc++ 22 do not ship it. When `#include <meta>` is not found at
  C++26, a note names the GCC installation (or libc++) kiln selected, and
  a GCC 16 found through `g++-16` or `g++` on `PATH`.
- C++26 contracts (P2900): `pre`, `post`, `contract_assert`, the four
  evaluation semantics (`-fcontract-evaluation-semantic=`), and a
  replaceable violation handler.
- C++26 erroneous behaviour (P2795): at `-std=c++26` an uninitialized
  automatic variable (up to 4096 bytes) is zero-filled at its
  declaration. clang 22 does not do this. `[[indeterminate]]` on the
  variable, or `-ftrivial-auto-var-init=uninitialized`, opts out.
- C++20 modules, header units and `import std;`, in one invocation or in
  clang's two-step build: `--precompile`, `-fmodule-output`,
  `-fmodule-file=`, `-fprebuilt-module-path=`. kiln's module files are
  its own format. The declaration image (`-fkiln-bmi-image`, which builds
  an imported module from a serialized image instead of replaying it) is
  experimental and off by default; it does not yet cover `import std;`.
- Dependency scanning: `kiln-scan-deps` (also installed as
  `clang-scan-deps-22` and `clang-scan-deps`) writes P1689r5, so CMake
  3.28+ with Ninja builds C++20 module targets unchanged.
- Coroutines, exceptions, RTTI, `thread_local`, and clang's and GCC's
  common extensions and builtins.

### Constant evaluation (the Oven)

- All constant evaluation (`constexpr`, `consteval`, `static_assert`,
  template arguments, reflection) runs in the Oven, kiln's bytecode
  evaluator. It has exact per-evaluation budgets for steps, depth and
  live memory (`-fconstexpr-steps=`, `-fconstexpr-depth=`,
  `-fconstexpr-memory=`), and memory freed during an evaluation is
  reclaimed during that evaluation.
- On kiln's benchmark corpus, compared with clang 19: geometric mean
  0.21x the time and 0.17x the peak memory
  ([docs/status/perf.md](docs/status/perf.md)).
- Reports: `-fconstexpr-report=` (NDJSON per evaluation),
  `-fkiln-instantiation-report`, `-fkiln-memory-report`.

### Standard libraries and compatibility

- libc++ 22.1.8 test suite at `-std=c++26`: 99.4%. libc++ 19.1.7: 98.3%.
  Every public libstdc++ 14 header compiles at `-std=c++20` and
  `-std=c++26`.
- 47 open-source projects built with their own build systems and test
  suites, including all of LLVM, Boost, folly, Abseil, DuckDB and CMake:
  46 at parity with clang 19 or better on every test; the Boost
  metaprogramming libraries match clang 22.
- clang-compatible command line and predefined macros. CMake and
  autotools work unchanged. kiln reproduces clang 22 by default and
  clang 19 with `-fclang-compat=19`.

### Code generation and tools

- Writes LLVM IR as text and drives LLVM's tools. It never links against
  LLVM. Supports LLVM 22 (preferred) and LLVM 19, found on its own
  (`--llvm-dir=` and `KILN_LLVM_DIR` name one); `--version` and `-v`
  print the LLVM in use.
- Itanium C++ ABI as clang implements it, on Linux aarch64 and x86_64.
- `-O0` to `-O3`, `-Os`, `-Oz`; LTO (`-flto`, `-flto=thin`);
  floating-point model flags; AArch64 branch protection.
- DWARF debug information (`-g` and its levels, `-gdwarf-N`) that gdb and
  lldb read as they read clang's.
- Sanitizers: address, hwaddress, thread, memory, leak, fuzzer, and
  undefined behaviour.
- Coverage: `--coverage` (gcov) and `-fprofile-instr-generate`
  counters. Coverage mapping and PGO are not in 1.0.

### Robustness and distribution

- An internal compiler error prints a report and writes a reproducer
  (`.ii` and `.sh`) instead of a panic
  ([docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md)). kiln runs out of
  stack only as a diagnostic.
- Every crate is `#![forbid(unsafe_code)]`. The MSRV is Rust 1.95.
- Release archives from `cargo xtask dist`: relocatable,
  `kiln-<version>-<triple>.tar.xz` with a stripped binary, plus a
  `-symbols` archive and `SHA256SUMS`.

### Fixed in the final release candidate

These landed after the first 1.0 release candidate was measured.

- x86 builtins: every x86 builtin that clang lowers to generic IR is now
  lowered (AVX and AVX2 extract, insert and `perm2x128`, F16C, the FMA
  family and more). The builtins kiln declares but cannot lower went
  from 595 to 23 (`maskmovq`, Key Locker, the AMX internals), and those
  23 answer `__has_builtin` with 0. A call of an x86 builtin sets
  `min-legal-vector-width`, which fixes an LLVM backend crash at
  `-march=skylake-avx512 -O2`. OpenCV's AVX2 dispatch TUs now compile
  (WP-1242).
- Reflection: `extract` of a function now instantiates and odr-uses it
  (WP-1244).
- Constant evaluation: an uncaught exception in a constant evaluation
  prints its `what()` whole, up to 64 KiB, after which it ends with
  "… [N more characters]" (WP-1245).
- Reflection: `^^` accepts operator, literal-operator, conversion and
  dependent template-ids, such as `^^ns::operator==<int>` (WP-1246).
- Reflection: `members_of` leaves out members whose type is an undeduced
  placeholder, and `type_of` on one throws, as in GCC.
  `display_string_of` prints requires-clauses as GCC 16.1 does
  (WP-1243).

## Before 1.0

Before 1.0 kiln was developed in the open on `main`, at version 0.0.1,
with no tagged releases. The history is in the git log, the milestones
in [docs/ROADMAP.md](docs/ROADMAP.md), and each work package's record in
[docs/BACKLOG.md](docs/BACKLOG.md) and `spec/landed/`.

[1.0.2]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.2
[1.0.1]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.1
[1.0.0]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.0
