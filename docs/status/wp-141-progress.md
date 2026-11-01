# WP-141 — panic sweep (kiln_parse, kiln_sema, kiln_oven, kiln_codegen)

Rule (docs/AGENTS.md item 5): anything derivable from user input is a
diagnostic, never a panic. This file records the sweep of `panic!`,
`unwrap()`, `expect(`, `unreachable!`, `assert!`, `todo!`, `unimplemented!`
and "not yet implemented" in the four crates' non-test code (inline
`#[cfg(test)] mod … { }` blocks and `tests.rs` excluded), as of branch
`wp/141-no-panics`.

Classes: **(a)** an internal invariant (the input cannot reach it, or only
through a bug elsewhere); **(b)** reachable from user input (with the input
shape).

## Result

No (b) site remains in the four crates. The two reachable panics the M3
gate hit were `kiln_ast` assertions reached through `kiln_sema`, both fixed
here (see "Fixed"). `todo!` and `unimplemented!` have no occurrences.

| crate | sites | (a) | (b) | notes |
|---|---:|---:|---:|---|
| kiln_parse | 2 (+16) | 2 | 0 | the 16 `self.expect(Punct::…)` matches are the parser's token check (a diagnostic), not `Option::expect` |
| kiln_sema | 7 | 7 | 0 | plus 70 "not yet implemented" call sites, all diagnostics (`Sema::templ_unimplemented`, `Substituter::unimplemented`) |
| kiln_oven | 3 | 3 | 0 | |
| kiln_codegen | 157 | 157 | 0 | runs only on an error-free TU (below) |

## (a) internal invariants

kiln_parse
- `lib.rs:171` `assert!` — API precondition of `parse_tu`: the AST and the
  preprocessor share one interner (driver wiring, not input).
- `expr.rs:261` `unreachable!` — `?` is parsed with its middle operand; the
  `(None, None)` pair cannot be built by `parse_rhs`.

kiln_sema
- `literal.rs:361`, `expr.rs:218` `expect("… constants intern")` — the
  constant pool's `push_scalar` fails only past 2^32 entries (an address-space
  limit hit long before; the memory cap reports it).
- `expr.rs:3149` `expect("checked above")` — `as_record` of a type the
  preceding branch proved is a class.
- `oven_bridge.rs:103, 286, 302` `unreachable!` — the Oven is taken out of
  `Sema` only for the duration of one evaluation/`ensure_body`; re-entrant
  evaluation during instantiation goes through `ensure_body`, which puts it
  back (probed: static_assert / constexpr locals / VLA bounds inside a
  template instantiated mid-evaluation).
- `overload/ics.rs:167` `const _: () = assert!(…)` — compile-time size check.

kiln_oven
- `vm.rs:124` — compile-time size check.
- `kbc.rs:36` `panic!("invalid KBC opcode")` — decodes bytecode the Oven's
  own compiler emitted.
- `arith.rs:111` `unreachable!` — the compiler only routes integer binary
  ops here.

kiln_codegen (157: 142 `expect`, 9 `unreachable!`, 6 `panic!`)
- Precondition: the driver calls `kiln_codegen::emit` only when the
  diagnostic engine has no errors (`kiln_driver/src/codegen_cmd.rs:187`), so
  every site is an invariant of Sema's *accepted* output (spec/codegen.md),
  not of the input. By group:
  - type-shape `expect`s on a well-typed AST (`"pointee"`, `"field type"`,
    `"param type"`, `"method class"`, `"destructor type"`, …): expr.rs,
    class.rs, func.rs, lambda.rs, vtable.rs, new_delete.rs, try_catch.rs,
    eh.rs, consts.rs, module.rs;
  - statement context that Sema already diagnoses (`"break outside a
    loop"`, `"case outside a switch"`, `"case value folds"`, `"return
    slot"`): stmt.rs:247–654;
  - FIR builder invariants (insertion point, placeholders, use before
    definition, phi operands): fir.rs:479–710, plus `"insertion point"`
    `expect`s in class.rs, expr.rs, memptr.rs, vtable.rs, cleanup.rs;
  - cleanup / EH scope-stack discipline: cleanup.rs:160–996,
    try_catch.rs:78–138.
- A (b) here would mean Sema accepted an ill-formed program or built a
  malformed node; such a crash is a Sema bug and is fixed there.

## Fixed (b): the M3 gate crashes

| site | input shape | fix |
|---|---|---|
| `kiln_ast/src/decl.rs:1907` (`function_mut`: "decl has no Function payload"), via `templ/subst/decls.rs` | a friend function template defined in a class template, instantiated (`template<class T> struct A { template<class U> friend bool g(A, U) { return true; } }; A<int> a;`) — every TU including `<string>` with `-ferror-limit=0` (214 gate tests) | the friend's `FunctionTemplate` resolves to its pattern `FunctionDecl`; the instantiated friend template links to its pattern so its specializations get bodies |
| `kiln_ast/src/decl.rs:2234` (`mark_function`: "FnSpec bit is not settable after creation"), via `templ/subst/decls.rs` | a pure virtual member of a class template, instantiated (`template<class T> struct A { virtual void f() = 0; }; static_assert(__is_abstract(A<int>));`) | `pure` is set at creation through `Declarator.pure` (`FnSpec::PURE` is not a late bit) |
| "not yet implemented: substitution into RecoveryExpr" (diagnostic, not a panic) | an error in a template definition (`template<class T> int f(T t) { return t + nosuch; } int a = f(1);`) | a fresh `RecoveryExpr` is substituted; a failed statement stays erroneous; the Oven fails silently on an erroneous expression |

## Outside the sweep

`kiln_ast` (not in this WP's scope) has 180 such sites, chiefly the payload
getters' `assert!` (`decl.rs:1820`) and `mark_function`. They are where a
Sema bug surfaces as a panic, and both gate crashes were of this kind; the
`tests/stress/no-crash-libcxx-headers.cpp` entry (`expect: no-crash`, new in
spec/harness.md) guards the libc++ headers against them.
