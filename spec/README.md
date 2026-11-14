# Specs

One contract per crate (or large module). A spec says what the component
does, its public interface, invariants, error behavior, and which tests back
it. Standard references use stable section names, e.g. [lex.phases], from the
C++26 working draft (N5008 or later; use https://eel.is/c++draft/ for lookup).

A spec is normative for implementers: if code and spec disagree, fix one and
say which in the PR. A spec change without tests is incomplete.

| Spec | Crate | Status |
|---|---|---|
| [source.md](source.md) | kiln_source | draft |
| [snap.md](snap.md) | kiln_snap, kiln_snap_derive | prototype (WP-1377) |
| [pch.md](pch.md) | kiln_driver (`pch`, `pch_snapshot`), kiln_pp (`bmi` sections, lookup log) | implemented (1.2.0; WP-1397) |
| [diag.md](diag.md) | kiln_diag | draft |
| [lex.md](lex.md) | kiln_lex | draft |
| [pp.md](pp.md) | kiln_pp | draft |
| [oven.md](oven.md) | kiln_oven | design |
| [harness.md](harness.md) | xtask (`cargo xtask test`) | implemented |
| [bench.md](bench.md) | xtask (`cargo xtask bench`) | implemented |
| [clang-neg.md](clang-neg.md) | xtask (`cargo xtask clang-neg`) | implemented (WP-912) |
| [libstdcxx.md](libstdcxx.md) | xtask (`cargo xtask libstdcxx`) | implemented (WP-188 phase 1) |
| [types.md](types.md) | kiln_ast (`TypeContext`) | design |
| [ast.md](ast.md) | kiln_ast | design |
| [ast-dump.md](ast-dump.md) | kiln_ast (dump), xtask (`clang-ast`) | design |
| [abi.md](abi.md) | kiln_abi | implemented (M2: no bases) |
| [sema-conv.md](sema-conv.md) | kiln_sema (`conv`, `literal`) | implemented |
| [sema-overload.md](sema-overload.md) | kiln_sema (`overload`) | design (WP-S3O; WP-068a/b) |
| [llvm.md](llvm.md) | kiln_llvm | implemented |
| [parse.md](parse.md) | kiln_parse | implemented (M2); M3+ listed |
| [sema.md](sema.md) | kiln_sema | implemented (M2); M3+ listed |
| [sema-class.md](sema-class.md) | kiln_sema (classes), kiln_abi (layout, vtables) | design (WP-S3C) |
| [sema-templates.md](sema-templates.md) | kiln_sema (`templ`), kiln_parse, kiln_ast (templates) | design |
| [sema-lambda.md](sema-lambda.md) | kiln_parse, kiln_sema (`lambda`), kiln_oven, kiln_codegen (lambdas) | landed (WP-075) |
| [consteval-blocks.md](consteval-blocks.md) | kiln_parse, kiln_sema (`consteval_block`, `consteval`), kiln_oven (plain bit) | implemented (WP-473) |
| [reflection-repr.md](reflection-repr.md) | kiln_ast (`reflect`), kiln_parse, kiln_sema (`reflect_operand`, `consteval_only`) | implemented (WP-472) |
| [annotations.md](annotations.md) | kiln_parse (`attr`), kiln_sema (`annot`), kiln_ast (`annot`) | implemented (WP-481; queries M6-09) |
| [contracts.md](contracts.md) | kiln_parse (`contracts`), kiln_sema (`contracts`), kiln_ast (`ContractAssertStmt`), kiln_oven, kiln_codegen | parsed and analysed (WP-706); evaluation WP-707 |
| codegen, driver | | in progress (M2) |

## Landed sections

A work package that lands behaviour records it in a "Landed (WP-NNN)"
section: what the code now does, the rules it follows, and the tests
that back it. Since WP-488 a new one is **its own file**,
`spec/landed/<spec>/WP-NNN.md`, where `<spec>` is the stem of the spec it
extends (`codegen` for `codegen.md`, `abi` for `abi.md`, `sema-class`
for `sema-class.md`, …). The file's first line is `# WP-NNN — <title>`;
the body has the same shape a section in the spec had (rules, standard
and clang references, tests by path). Appending `## 7x. Landed …` /
`### 8.NN Landed …` to the end of a spec made every two open pull
requests conflict on that file, and every landing forced a re-merge
and a full CI re-run of the others; one file per WP never conflicts.

- The existing sections stay where they are (no migration). A spec
  whose Landed chapter has moved on ends that chapter with a pointer to
  its directory (the unnumbered "Landed sections after WP-488" in
  `codegen.md` and `abi.md`).
- The files are as normative as the spec they extend, and are read in
  WP order after the spec's own Landed chapter. A later WP that changes
  a rule an earlier file states updates that file (or, for a section
  still in the spec, the spec) rather than contradicting it.
- Section numbers are no longer allotted for Landed sections; refer to
  one as `spec/landed/codegen/WP-NNN.md`.
- `cargo xtask ci` checks, before anything is built (spec/harness.md
  "CI", step 0), that every file under `spec/landed/` is
  `spec/landed/<spec>/WP-NNN.md` for an existing `spec/<spec>.md`, that
  its first line is `# WP-NNN — <title>` with its own id, and that
  `docs/BACKLOG.md` has a `| WP-NNN |` row (`xtask/src/landed.rs`).
  `spec/landed/README.md` is the only other file allowed there.
