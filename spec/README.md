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
| [diag.md](diag.md) | kiln_diag | draft |
| [lex.md](lex.md) | kiln_lex | draft |
| [pp.md](pp.md) | kiln_pp | draft |
| [oven.md](oven.md) | kiln_oven | design |
| [harness.md](harness.md) | xtask (`cargo xtask test`) | implemented |
| [bench.md](bench.md) | xtask (`cargo xtask bench`) | implemented |
| [types.md](types.md) | kiln_ast (`TypeContext`) | design |
| [ast.md](ast.md) | kiln_ast | design |
| [ast-dump.md](ast-dump.md) | kiln_ast (dump), xtask (`clang-ast`) | design |
| [abi.md](abi.md) | kiln_abi | implemented (M2: no bases) |
| [sema-conv.md](sema-conv.md) | kiln_sema (`conv`, `literal`) | implemented |
| [sema-overload.md](sema-overload.md) | kiln_sema (`overload`) | design (WP-S3O; WP-068a/b) |
| [llvm.md](llvm.md) | kiln_llvm | implemented |
| [parse.md](parse.md) | kiln_parse | implemented (M2); M3+ listed |
| [sema.md](sema.md) | kiln_sema | implemented (M2); M3+ listed |
| [sema-templates.md](sema-templates.md) | kiln_sema (`templ`), kiln_parse, kiln_ast (templates) | design |
| codegen, driver | | in progress (M2) |
