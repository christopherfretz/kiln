# C++23 / C++26 language feature table (2026-11-01)

One row per core-language paper on the M4 list ([ROADMAP.md](../ROADMAP.md) M4),
plus DR-level papers. This is the M4 exit checklist: every C++23 row must be
`done`, and every C++26 row that clang 22 ships must be `done` or carry a
written reason. `cargo xtask features` checks the table mechanically (format
and checks: [spec/harness.md](../../spec/harness.md), "Feature table").

Columns:

- **Macro**: `` `__cpp_name` KILN / CLANG22 ``. KILN is `kiln++ -std=c++26 -dM -E`;
  CLANG22 is `clang++-22 -std=c++26 -dM -E -x c++ /dev/null` (22.1.8). `—` means
  the macro is not defined; a bare `—` means the paper has no `__cpp_*` macro of
  its own. Where clang 19 differs from clang 22, its value is in Notes (kiln's
  predefines are generated from clang 19, so they match it except where
  `apply_flags` raises a macro for a feature kiln implements: P0847's
  `__cpp_explicit_this_parameter` from C++23, P1061, P2893, P2747, P3068, P1306; the
  WP-357 ones are `KILN_FEATURES` in `crates/kiln_pp/src/predef.rs`).
- **Status**: `done` = kiln passes corpus tests covering the core forms;
  `partial` = some pass, or some are `xfail(kiln)`; `todo` = nothing passes.
  Seeded on 2026-11-01 (WP-185) from a corpus run through kiln.
- **Corpus tests**: paths under `tests/`. `*-probe.cpp` files are WP-185's
  one-per-paper probes; a probe marked `xfail(clang)` names the clang
  versions that lack the feature in its reason. Since the oracle switch
  (WP-355) a probe that only clang 19 lacks says `requires: clang>=22`
  instead, so it is a real two-compiler test under the clang 22 oracle and
  runs kiln at `-fclang-compat=22` under the clang 19 one.

## C++23

| Paper | Feature | `__cpp_*` macro (kiln / clang 22) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| P0847R7 | Deducing `this` (explicit object parameters) | `__cpp_explicit_this_parameter` 202110L / 202110L | done | `conformance/dcl.fct/explicit-object-call.cpp`<br>`conformance/dcl.fct/explicit-object-qualifiers.cpp`<br>`conformance/dcl.fct/explicit-object-diags.cpp`<br>`conformance/templates/deduce/temp.deduct.call-member.cpp`<br>`conformance/dcl.fct/p0847-lambda-probe.cpp`<br>`conformance/dcl.fct/p0847-lambda-explicit-object.cpp`<br>`conformance/dcl.fct/p0847-lambda-explicit-object-run.cpp`<br>`conformance/dcl.fct/p0847-lambda-explicit-object-diags.cpp` | Lambdas with an explicit object parameter, including the [expr.prim.lambda.closure]/5 closure-type check (WP-356). kiln defines the macro from C++23, as clang 22 does; clang 19: macro not defined (libc++ 19 keys `_LIBCPP_HAS_EXPLICIT_THIS_PARAMETER` on clang >= 18 instead). Also `dcl.fct/p0847-lambda-explicit-object-star.cpp` (`[*this]`, `requires: clang>=22`: clang 19 crashes on it; list it here once the oracle is clang 22). |
| P1938R3 | `if consteval` | `__cpp_if_consteval` 202106L / 202106L | done | `conformance/stmt.if/consteval-if-requires-compound.cpp`<br>`conformance/dcl.constexpr/consteval-run.cpp`<br>`conformance/ast/stmt.if/if-constexpr-consteval.cpp`<br>`conformance/ast/dcl.constexpr/consteval-if.cpp` | |
| P1169R4 | `static operator()` | `__cpp_static_call_operator` 202207L / 202207L | done | `conformance/over.oper/static-call.cpp`<br>`conformance/ast/over.oper/static-call.cpp`<br>`conformance/expr.prim.lambda/static-lambda.cpp` | |
| P2589R1 | `static operator[]` | — | done | `conformance/over.oper/static-call.cpp`<br>`conformance/ast/over.oper/static-call.cpp` | Bumps `__cpp_multidimensional_subscript` to 202211L (recorded on P2128). |
| P2128R6 | Multidimensional subscript operator | `__cpp_multidimensional_subscript` 202211L / 202211L | done | `conformance/over.oper/static-call.cpp`<br>`conformance/over.match.oper/selection.cpp` | P2128 alone is 202110L. |
| P0849R8 | `auto(x)` and `auto{x}` decay-copy | `__cpp_auto_cast` 202110L / 202110L | done | `conformance/expr.type.conv/p0849-probe.cpp`<br>`conformance/expr.type.conv/p0849-auto-cast.cpp`<br>`conformance/expr.type.conv/p0849-auto-cast-diags.cpp`<br>`conformance/expr.type.conv/p0849-auto-cast-cxx20.cpp` | WP-356. |
| P1774R8 | `[[assume]]` | — | done | `conformance/dcl.attr.assume/assume-diags.cpp`<br>`conformance/dcl.attr.assume/assume-comma-unused.cpp`<br>`conformance/dcl.attr.assume/assume-pre-cxx23-ext.cpp`<br>`conformance/expr.const/assume-false.cpp`<br>`conformance/ast/dcl.attr.assume/assume-attributed-stmt.cpp` | Detected with `__has_cpp_attribute(assume)`; no `__cpp_*` macro. |
| P2334R1 | `#elifdef` and `#elifndef` | — | done | `conformance/cpp.cond/p2334-probe.cpp` | |
| P2437R1 | `#warning` | — | done | `conformance/cpp.error/warning.h` | |
| P1467R9 | Extended floating-point types | — | done | `conformance/basic.extended.fp/p1467-probe.cpp` | detected via `__STDCPP_*_T__`. WP-361: `_Float32`/`_Float64`/`_Float128`/`__bf16` (with `_Float16`), suffixes, ranks, GCC 14 macros, libstdc++ `<stdfloat>` (`basic.extended.fp/p1467-*.cpp`, `libstdcxx-stdfloat.cpp`). Neither clang 19 nor clang 22 implements it (tests `xfail(clang)`). |
| P2718R0 | Lifetime extension in range-based `for` | `__cpp_range_based_for` 202211L / 202211L | done | `conformance/stmt.ranged/p2718-order.cpp`<br>`conformance/stmt.ranged/p2718-constexpr.cpp` | |
| P2448R2 | Relaxing some constexpr restrictions | — | done | `conformance/dcl.constexpr/p2448-probe.cpp` | `__cpp_constexpr` 202207L; that macro is recorded on P2747, its latest bump. |
| P2242R3 | Non-literal variables, labels and `goto` in constexpr functions | — | done | `conformance/dcl.constexpr/p2242-probe.cpp` | `__cpp_constexpr` 202110L (see P2747). |
| P2280R4 | Unknown references in constant expressions | — | done | `conformance/expr.const/p2280-probe.cpp`<br>`conformance/expr.const/p2280-unknown-reference.cpp`<br>`conformance/expr.const/p2280-unknown-reference-invalid.cpp`<br>`conformance/expr.const/p2280-unknown-reference-cxx20.cpp`<br>`conformance/expr.const/p2280-ok.cpp`<br>`conformance/expr.const/p2280-errors.cpp`<br>`conformance/expr.const/p2280-unknown-reference-mem.cpp` | DR, applied from C++23 as clang does (C++20 keeps the old rule). clang 19 lacks it (`requires: clang>=22`). WP-357; the unknown object holds no bytes (WP-375). |
| P2647R1 | `static constexpr` variables in constexpr functions | — | done | `conformance/dcl.constexpr/p2647-probe.cpp` | `__cpp_constexpr` 202211L (see P2747). |
| P2582R1 | CTAD from inherited constructors | — | done | `conformance/over.match.class.deduct/p2582-probe.cpp`<br>`conformance/over.match.class.deduct/p2582-inherited-guides.cpp`<br>`conformance/over.match.class.deduct/p2582-not-deducible.cpp`<br>`conformance/over.match.class.deduct/run-p2582.cpp` | Neither clang implements it (clang 19 and 22 reject the paper's examples; probe `xfail(clang)`, the other tests `skip(clang)`). Its macro bump, `__cpp_deduction_guides` 202207L, is absent from both clangs (201703L) and kiln keeps 201703L to match them. Not yet: `using G::F::F` naming the base through the injected-class-name, guides the base itself inherited, and an alias template of an inheriting class (spec/sema-templates.md §6.6.1). |
| P2324R2 | Labels at the end of compound statements | — | done | `conformance/stmt.label/p2324-probe.cpp` | ROADMAP M4 cites this as "P2360 labels at the end of blocks"; P2324 is the labels paper. |
| P0330R8 | `size_t` literal suffixes | `__cpp_size_t_suffix` 202011L / 202011L | done | `conformance/lex.icon/p0330-probe.cpp`<br>`conformance/ast/lex.icon/integer-literals.cpp` | ROADMAP M4 cites this as "P2324 `size_t` literals"; the paper is P0330. |
| P2360R0 | Alias-declaration in an init-statement | — | done | `conformance/stmt.pre/p2360-probe.cpp`<br>`conformance/stmt.pre/p2360-for-probe.cpp`<br>`conformance/stmt.pre/p2360-for-alias.cpp`<br>`conformance/stmt.pre/p2360-for-alias-cxx20.cpp` | Range-based `for` form added by WP-356. |
| P2290R3 | Delimited escape sequences | — | done | `conformance/lex.ccon/p2290-probe.cpp`<br>`differential/pp/torture/lex.literal-forms.cpp` | |
| P2071R2 | Named universal character escapes | `__cpp_named_character_escapes` 202207L / 202207L | done | `conformance/lex.charset/p2071-probe.cpp` | |
| P2295R6 | UTF-8 as a portable source encoding | — | done | `conformance/lex.phases/p2295-probe.cpp` | |
| P2362R3 | Ill-formed multicharacter and non-encodable wide literals | — | done | `conformance/lex.ccon/p2362-probe.cpp` | |
| P2201R1 | Mixed string literal concatenation | — | done | `conformance/lex.string/p2201-probe.cpp` | |
| P2156R1 | Duplicate attributes | — | done | `conformance/dcl.attr.grammar/p2156-probe.cpp` | |
| P2266R3 | Simpler implicit move | `__cpp_implicit_move` 202207L / 202207L | done | `conformance/dcl.init.ref/implicit-move-return.cpp` | |
| P2468R2 | The equality operator you are looking for | — | done | `conformance/class.compare/ambiguous-reversed.cpp`<br>`conformance/class.compare/rewritten-selection.cpp`<br>`conformance/over.match.oper/rewritten-template-errors.cpp` | DR. |
| P2493R0 | Missing feature-test macros for C++20 core papers | — | done | `conformance/cpp.predefined/p2493-probe.cpp` | |
| P1102R2 | Optional `()` for lambdas | — | done | `conformance/expr.prim.lambda/p1102-probe.cpp` | |
| P2036R3 | Lambda trailing-return-type scope | — | done | `conformance/expr.prim.lambda/p2036-probe.cpp`<br>`conformance/expr.prim.lambda/p2036-init-capture-scope.cpp`<br>`conformance/expr.prim.lambda/p2036-init-capture-scope-diags.cpp` | DR (also C++20 mode), WP-356. Init-captures are visible in the trailing return type and `noexcept`; clang 19/22 also skip the const-qualified `decltype((x))` part. |
| P2173R1 | Attributes on lambda expressions | — | done | `conformance/expr.prim.lambda/attributes.cpp` | |
| P2564R3 | `consteval` needs to propagate up | `__cpp_consteval` 202211L / 202211L | done | `conformance/dcl.constexpr/consteval-escalation.cpp`<br>`conformance/dcl.constexpr/consteval-lambda.cpp`<br>`conformance/dcl.constexpr/consteval-generic-lambda-reason.cpp`<br>`conformance/expr.prim.lambda/consteval-conversion.cpp` | DR. |
| P2513R4 | `char8_t` compatibility fix | `__cpp_char8_t` 202207L / 202207L | done | `conformance/dcl.init.string/p2513-probe.cpp` | DR. |
| P2327R1 | De-deprecating volatile compound operations | — | done | `conformance/expr.ass/p2327-probe.cpp` | DR; CWG2654 extends it to every compound assignment. kiln does not emit P1152's `-Wdeprecated-volatile` for `++`/`--` (follow-up). |
| P1401R5 | Narrowing contextual conversions to `bool` | — | done | `conformance/dcl.pre/p1401-probe.cpp` | |

## C++26 (non-reflection)

| Paper | Feature | `__cpp_*` macro (kiln / clang 22) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| P2662R3 | Pack indexing | `__cpp_pack_indexing` 202311L / 202311L | done | `conformance/templates/packs/temp.variadic-pack-index-expr.cpp`<br>`conformance/templates/packs/temp.variadic-pack-index-type.cpp`<br>`conformance/dcl.type.pack.index/nested-name-specifier.cpp` | |
| P2169R4 | Placeholder variables `_` | `__cpp_placeholder_variables` 202306L / 202306L | done | `conformance/basic.scope.scope/p2169-probe.cpp`<br>`conformance/basic.scope.scope/p2169-placeholder.cpp`<br>`conformance/basic.scope.scope/p2169-placeholder-diags.cpp`<br>`conformance/basic.scope.scope/p2169-placeholder-cxx23.cpp` | WP-356. |
| P2573R2 | `= delete("reason")` | `__cpp_deleted_function` 202403L / 202403L | done | `conformance/dcl.fct.def.delete/redeclaration.cpp`<br>`conformance/dcl.fct.def.delete/templates.cpp`<br>`conformance/dcl.fct.def.delete/uses.cpp` | |
| P2741R3 | User-generated `static_assert` messages | `__cpp_static_assert` 202306L / 202306L | done | `conformance/dcl.pre/p2741-probe.cpp`<br>`conformance/dcl.pre/p2741-static-assert-message.cpp`<br>`conformance/dcl.pre/p2741-static-assert-message-ok.cpp`<br>`conformance/dcl.pre/p2741-static-assert-message-diags.cpp`<br>`conformance/dcl.pre/p2741-static-assert-message-cxx23.cpp` | WP-356; the message is evaluated by the Oven. Diagnostics follow clang 22 (`requires: clang>=22`). |
| P2893R3 | Variadic friends | `__cpp_variadic_friend` 202403L / 202403L | done | `conformance/class.friend/p2893-probe.cpp`<br>`conformance/class.friend/p2893-variadic-friend.cpp`<br>`conformance/class.friend/p2893-access-errors.cpp`<br>`conformance/class.friend/p2893-diags.cpp`<br>`conformance/class.friend/p2893-ext.cpp` | clang 19 lacks it: macro not defined; the probe `requires: clang>=22` (WP-355). kiln defines the macro in every C++ mode, as clang 22 does. |
| P2747R2 | `constexpr` placement new | `__cpp_constexpr` 202406L / 202406L | done | `conformance/expr.new/p2747-probe.cpp`<br>`conformance/expr.new/p2747-placement-new.cpp`<br>`conformance/expr.new/constexpr-placement.cpp`<br>`conformance/expr.new/constexpr-placement-ok.cpp`<br>`conformance/expr.new/p2747-placement-churn.cpp`<br>`conformance/expr.new/p2747-placement-voidified-subobject.cpp` | clang 19: 202306L, the P2738 level, with no P2747 (tests `requires: clang>=22`). WP-357, WP-375, WP-378. |
| P3068R6 | `constexpr` exceptions | `__cpp_constexpr_exceptions` 202411L / — | done | `conformance/expr.throw/p3068-probe.cpp`<br>`conformance/expr.throw/constexpr-exceptions.cpp`<br>`conformance/expr.throw/constexpr-exceptions-template.cpp`<br>`conformance/expr.throw/constexpr-uncaught.cpp`<br>`conformance/expr.throw/constexpr-throw-churn.cpp`<br>`conformance/except.handle/constexpr-handler-match.cpp`<br>`conformance/except.handle/constexpr-function-try-block.cpp`<br>`conformance/except.ctor/constexpr-ctor-try-block-subobjects.cpp`<br>`conformance/expr.throw/constexpr-throw-ctor-throws.cpp`<br>`conformance/except.handle/constexpr-catch-virtual-base-ptr.cpp`<br>`conformance/except.handle/constexpr-catch-virtual-base-ptr-subobject.cpp`<br>`conformance/except.handle/constexpr-nested-evaluation.cpp`<br>`conformance/except.handle/constexpr-bcp-trials.cpp`<br>`conformance/except.handle/constexpr-unwound-lifetime.cpp`<br>`conformance/except.uncaught/constexpr-uncaught-exceptions.cpp`<br>`conformance/expr.throw/constexpr-throw.cpp` | Not in clang 22: written reason for the M4 exit; the tests are `skip(clang)`. `constexpr-throw.cpp` pins the pre-P3068 rejection at C++23. Language part only: `std::current_exception` and `__builtin_uncaught_exceptions` are not constexpr, `std::uncaught_exceptions` only when declared `constexpr` (P3842R2 withdrew it); libc++ 22 marks no `<exception>` member constexpr and does not test the macro. WP-357, WP-368, WP-383; spec/oven.md "Exceptions". |
| P3533R2 | `constexpr` virtual inheritance | — | done | `conformance/class.mi/constexpr-virtual-base-cxx26.cpp`<br>`conformance/class.mi/constexpr-virtual-base-cxx23.cpp`<br>`conformance/class.mi/constexpr-virtual-diamond-run.cpp`<br>`conformance/class.mi/constexpr-virtual-nested-run.cpp`<br>`conformance/class.derived/constexpr-virtual-base-conversions.cpp`<br>`conformance/class.derived/constexpr-virtual-base-conversions-cxx23.cpp`<br>`conformance/class.derived/virtual-base-downcast.cpp`<br>`conformance/class.inhctor.init/constexpr-vbase-run.cpp`<br>`conformance/except.handle/constexpr-catch-in-virtual-base-storage.cpp`<br>`stress/constexpr-virtual-deep-diamond.cpp` | Not in clang 22: written reason for the M4 exit; the C++26 tests are `skip(clang)`, and the C++23 rejections (`dcl.constexpr/virtual-base.cpp`, `dcl.constexpr/non-literal-type.cpp`, `expr.const/non-literal-local.cpp`, `class.inhctor.init/constexpr-vbase.cpp`) are pinned to `std: c++23`. The paper bumps `__cpp_constexpr` to 202502L; clang 22 defines 202406L, and since `__cpp_constexpr` is a shared macro whose value also claims every earlier constexpr paper, kiln follows clang's value until clang ships P3533 (decision WP-388), so the macro stays on the P2747 row. In C++23 mode an explicitly-defaulted `constexpr` default constructor or destructor of a class with a direct virtual base follows clang's running `DefaultedDefaultConstructorIsConstexpr` flag (spec/sema-class.md §4.6, WP-400). WP-388; spec/oven.md "Virtual bases". |
| P2738R1 | `constexpr` cast from `void*` | — | done | `conformance/expr.static.cast/p2738-probe.cpp` | `__cpp_constexpr` 202306L (see P2747). |
| P1967R14 | `#embed` | `__cpp_pp_embed` — / — | done | `conformance/cpp.embed/p1967-probe.cpp`<br>`differential/pp/torture/cpp.embed-params.cpp` | Neither clang defines `__cpp_pp_embed`; use `__has_embed`. clang reports `#embed` in C++ as a C23 extension (`-Wc23-extensions`), and kiln matches. |
| P0963R3 | Structured binding declaration as a condition | — | done | `conformance/dcl.struct.bind/condition.cpp`<br>`conformance/dcl.struct.bind/p0963-binding-condition-cxx23.cpp` | WP-356: no warning in C++26, clang 19's `-Wbinding-in-condition` warning before (clang 22 words it as a C++2c extension); tuple-like bindings are initialized after the test (`dcl.struct.bind/p0963-binding-condition.cpp`, `requires: clang>=22`; list it here once the oracle is clang 22). A `switch` condition of class type is not supported yet (kiln has no class-to-integral conversion for `switch`). Its `__cpp_structured_bindings` bump (202406L) is subsumed by P1061's 202411L, which kiln defines. |
| P1061R10 | Structured bindings can introduce a pack | `__cpp_structured_bindings` 202411L / 202411L | done | `conformance/dcl.struct.bind/pack.cpp`<br>`conformance/dcl.struct.bind/pack-positive.cpp`<br>`conformance/dcl.struct.bind/pack-errors.cpp`<br>`conformance/dcl.struct.bind/pack-ext.cpp`<br>`conformance/dcl.struct.bind/run-pack-fold.cpp` | clang 19 lacks it (202403L, the P0609 level); kiln raises the macro to 202411L wherever it is defined (C++17 on), as clang 22 does. |
| P0609R3 | Attributes for structured bindings | — | done | `conformance/dcl.struct.bind/p0609-probe.cpp` | Supplies `__cpp_structured_bindings` 202403L, now superseded by P1061's 202411L. |
| P2795R5 | Erroneous behaviour for uninitialized reads | — | done | `conformance/dcl.attr.indet/p2795-probe.cpp`<br>`conformance/dcl.attr.indet/appertainment.cpp`<br>`conformance/dcl.attr.indet/param.cpp`<br>`conformance/dcl.attr.indet/cxx23-unknown.cpp`<br>`conformance/dcl.attr.indet/has-cpp-attribute.cpp`<br>`conformance/basic.indet/run-erroneous-zero.cpp`<br>`conformance/basic.indet/run-byte-read.cpp`<br>`conformance/basic.indet/run-threshold.cpp`<br>`conformance/basic.indet/indeterminate-compiles.cpp`<br>`conformance/basic.indet/ir-cxx23-unchanged.cpp`<br>`conformance/basic.indet/constexpr-erroneous.cpp` | Not in clang 22 (no `[[indeterminate]]`): the tests are `requires: clang>=23`. kiln's erroneous value is all-zero bytes, written at the declaration of an automatic variable of at most 4096 bytes under C++26 (`-ftrivial-auto-var-init=zero` IR; `codegen/erroneous_init.cpp`, `kiln_driver/tests/erroneous_init.rs`); `__has_cpp_attribute(indeterminate)` is 202403L. Erroneous and indeterminate reads stay non-constant in the Oven. WP-448; spec/codegen.md §7r, spec/sema.md §4.5, spec/oven.md. |
| P2786R13 | Trivial relocatability | `__cpp_trivial_relocatability` — / 202502L | partial | `conformance/class.prop/p2786-probe.cpp` | clang 19 lacks it: macro not defined; the probe `requires: clang>=22` (WP-355). kiln has `__builtin_is_cpp_trivially_relocatable` (clang 22 semantics, WP-263) and, at `-fclang-compat=22`, clang 22's `-Wdeprecated-builtins` warning on `__is_trivially_relocatable` (`conformance/ext.traits/is-trivially-relocatable-deprecated.cpp`); the macro stays undefined. Not in C++26: adopted at Hagenberg (Feb 2025), removed from the working draft at the Kona plenary (Nov 2025) after national-body comments and a design defect that could not be fixed in time; deferred to C++29 (open design questions: [P4197R0](https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2026/p4197r0.html); [Kona trip report](https://herbsutter.com/2025/11/10/trip-report-november-2025-iso-c-standards-meeting-kona-usa/)). `partial` is kiln's coverage, not a target: this row is exempt from the M4 C++26 exit; do not implement relocation semantics until the C++29 paper settles (WP-367). |
| P1306R5 | Expansion statements | `__cpp_expansion_statements` 202506L / — | done | `conformance/stmt.expand/p1306-probe.cpp`<br>`conformance/stmt.expand/iterating.cpp`<br>`conformance/stmt.expand/iterating-neg.cpp`<br>`conformance/stmt.expand/destructuring.cpp`<br>`conformance/stmt.expand/destructuring-neg.cpp`<br>`conformance/stmt.expand/nested.cpp`<br>`conformance/stmt.expand/empty.cpp`<br>`conformance/stmt.expand/dependent.cpp`<br>`conformance/stmt.expand/cxx23-ext.cpp`<br>`conformance/stmt.expand/compound-body.cpp`<br>`conformance/stmt.expand/oven-memory.cpp`<br>`conformance/stmt.expand/ast-dump.cpp` | Not in clang 22 (clang 23 ships it): the tests are `requires: clang>=23`, so they run under kiln alone today; the run tests `enumerating.cpp`, `control.cpp` and `references.cpp` (runtime break/continue/return, lvalue elements) are SKIPs under the clang 22 oracle and are not listed. The macro is kiln's at C++26 at every `-fclang-compat` (see Macro gaps). All three forms, constexpr elements, nesting, dependent forms, N = 0; codegen `codegen/expansion_stmt.cpp` and `kiln_codegen` test `expansion_statement_lowers_in_order`. Deviations: the size N is counted by evaluating `begin + i != end`; P2686 (constexpr references to automatic variables) is not implemented, so a constexpr range or destructuring of a glvalue falls back to a copy, and user-written `constexpr` structured bindings stay rejected; a braced or designated element of an expansion-init-list is unsupported. WP-447; spec/sema.md "Expansion statements". |
| P2361R6 | Unevaluated strings | — | done | `conformance/lex.string.uneval/unevaluated-string-macro.cpp` | |
| P2864R2 | Removing deprecated arithmetic conversion on enumerations | — | done | `conformance/expr.arith.conv/p2864-probe.cpp` | |
| P2748R5 | Disallow binding a returned glvalue to a temporary | — | done | `conformance/class.temporary/return-dangling.cpp`<br>`conformance/stmt.return/stack-address-temporary.cpp` | |
| P3144R2 | Deleting a pointer to an incomplete type is ill-formed | — | done | `conformance/expr.delete/delete-incomplete.cpp` | |
| P2963R3 | Ordering of constraints involving fold expressions | — | done | `conformance/templates/concepts/temp.constr.fold-expanded.cpp` | |
| P2865R6 | Removing deprecated array comparisons | — | done | `conformance/expr.eq/p2865-probe.cpp`<br>`conformance/expr.eq/p2865-array-comparison.cpp`<br>`conformance/expr.eq/p2865-array-comparison-cxx20.cpp` | WP-356. clang 19 only warns (probe `xfail(clang)`). |
| P3176R1 | The Oxford variadic comma | — | done | `conformance/dcl.fct/p3176-probe.cpp`<br>`conformance/dcl.fct/p3176-variadic-comma.cpp`<br>`conformance/dcl.fct/p3176-variadic-comma-cxx23.cpp` | WP-356. clang 19 lacks the deprecation (probe `xfail(clang)`). |

## C++26 reflection (M6)

One row per M6 work package (docs/ROADMAP.md M6). The contract is
[spec/reflection.md](../../spec/reflection.md), and the papers are listed in
its §1. All rows need `-freflection` (§1.4). The reference compiler is GCC 16.1.
This table is per work package, not per paper, so `cargo xtask features` does
not check it: its first header cell is not `Paper`.

| WP | Scope | Status | Tests | Notes |
|---|---|---|---|---|
| WP-470 | M6-01: spec/reflection.md, the dispatch skeleton (op families, purity classes, `SemaBridge::meta`) | done | `conformance/reflection/dispatch-inline-ns.cpp`<br>`conformance/reflection/memo-purity.cpp` | The four spike ops are ported unchanged; `info` parameters are memo-eligible; `memo_hits` report counter. |
| WP-471 | M6-02: spike hardening (incomplete-class layout, `Sema::same_entity`, table overflow, type-table epoch) | todo | | |
| WP-472 | M6-03: representation and the `^^` operator (alias sugar, templates, namespaces, values) | done | `conformance/expr.reflect/global-namespace.cpp`<br>`conformance/expr.reflect/namespace.cpp`<br>`conformance/expr.reflect/namespace-alias.cpp`<br>`conformance/expr.reflect/type.cpp`<br>`conformance/expr.reflect/templates.cpp`<br>`conformance/expr.reflect/template-id-specializations.cpp`<br>`conformance/expr.reflect/enumerator.cpp`<br>`conformance/expr.reflect/structured-binding.cpp`<br>`conformance/expr.reflect/variable.cpp`<br>`conformance/expr.reflect/data-member.cpp`<br>`conformance/expr.reflect/member-function.cpp`<br>`conformance/expr.reflect/parameter.cpp`<br>`conformance/expr.reflect/template-member-remap.cpp`<br>`conformance/expr.reflect/equality-matrix.cpp`<br>`conformance/expr.reflect/alias-vs-dealias-identity.cpp`<br>`conformance/expr.reflect/rejected-operands.cpp`<br>`conformance/expr.reflect/nttp-info-kinds.cpp`<br>`conformance/expr.reflect/ast-dump.cpp`<br>`conformance/basic.types.general/consteval-only.cpp`<br>`conformance/basic.types.general/consteval-only-escalation-run.cpp` | Contract: spec/reflection-repr.md. Every `Reflectee` variant; `^^` for every operand kind except operator, conversion-function and destructor names and dependent qualified names / template-ids (unsupported diagnostics); a top-level alias is kept (`^^J != ^^I` for `using J = I;`, where GCC 16.1 says equal); `template <info R>` of every kind with a TU-local vendor mangling `Lu6__info<id>E`; the consteval-only variable, function and expression rules of §4.1 (gap: member functions of a consteval-only class). |
| WP-473 | M6-04: consteval blocks (P3289), plainly constant-evaluated, consteval-only types | todo | | |
| WP-474 | M6-05: splices in every position, including dependent ones | todo | | Type splices exist from the spike. |
| WP-475 | M6-06: result materialization, the names family, `std::meta::exception` (P3560) | todo | | |
| WP-476 | M6-07: queries and predicates, parameters (P3096) | done | `conformance/meta.reflection.queries/kinds.cpp`<br>`conformance/meta.reflection.queries/linkage-storage.cpp`<br>`conformance/meta.reflection.queries/functions.cpp`<br>`conformance/meta.reflection.queries/members.cpp`<br>`conformance/meta.reflection.queries/parent-dealias.cpp`<br>`conformance/meta.reflection.queries/templates.cpp`<br>`conformance/meta.reflection.queries/values-objects.cpp`<br>`conformance/meta.reflection.queries/operators.cpp`<br>`conformance/meta.reflection.queries/complete-enumerable.cpp`<br>`conformance/meta.reflection.queries/parameters-p3096.cpp`<br>`conformance/meta.reflection.queries/entity-neg.cpp`<br>`conformance/meta.reflection.queries/operators-parameters-neg.cpp`<br>`conformance/meta.reflection.queries/oven-budget.cpp` | Contract: spec/reflection-queries.md. Every `[meta.reflection.queries]` predicate and entity query except the template queries (WP-482); P3096 `variable_of`, `return_type_of`, `is_function_parameter`, `is_explicit_object_parameter`, `has_default_argument`, `is_vararg_function`. `parameters_of` is WP-478's, so the positive parameter tests are disabled. `constant_of`/`object_of` of class, array and reference variables are not implemented yet (WP-479). `symbol_of` returns `const char*` until WP-475. GCC 16.1 deviations are in spec/reflection-queries.md §5. |
| WP-477 | M6-08: type traits family | todo | | |
| WP-478 | M6-09: members, access contexts (P3547), layout, annotation queries | todo | | Includes implicit special members (D4). |
| WP-479 | M6-10: values and static storage (P3491), `refl_bytes` | todo | | |
| WP-480 | M6-11: `define_aggregate`, `data_member_spec`, `__kiln_define_member` rules (D7) | todo | | |
| WP-481 | M6-12: annotations (P3394), syntax and storage | todo | | |
| WP-482 | M6-13: `substitute`, `can_substitute`, template queries | todo | | |
| WP-483 | M6-14: validation and perf infrastructure (GCC differential, `bench/reflect`, report fields) | todo | | |
| WP-484 | M6-15: libstdc++ `<meta>` and the feature macros | todo | | |
| WP-485 | M6-16: reflection mangling and codegen of entities templated on `info` | todo | | |

## DR-level papers

DRs already listed above: P2280, P2468, P2036, P2564, P2513, P2327.

| Paper | Feature | `__cpp_*` macro (kiln / clang 22) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| P1949R7 | C++ identifier syntax using UAX 31 | — | done | `conformance/lex.name/p1949-probe.cpp` | |
| P2316R2 | Consistent character literal encoding | — | done | `conformance/cpp.cond/p2316-probe.cpp` | |
| P1854R4 | Making non-encodable string literals ill-formed | — | done | `conformance/lex.ccon/p1854-probe.cpp` | |
| P2615R1 | Meaningful exports | — | done | `conformance/module.interface/export-errors.cpp` | `export` of static_assert, using-directives, empty declarations and explicit specializations is accepted, as clang 22 (spec/modules.md §4). |
| P2788R0 | Linkage for modular constants | — | done | `mangle/modules.cpp` | A `const` variable attached to a named module has module linkage (`_ZW1M1c`), differential against clang. |
| P3034R1 | Module declarations shouldn't be macros | — | done | `conformance/cpp.module/p3034-probe.cpp`<br>`conformance/cpp.module/p3034-function-like.cpp` | clang 19 and 22 expand object-like macros in module names (probe `xfail(clang)`); kiln diagnoses them per the paper. |

Not tabulated:
- Contracts (P2900): clang 22 does not ship them.
- P2300-adjacent library hooks: the libc++ status table covers them.
- Codegen-only DRs with no front-end observable (P2809 trivial infinite loops, P2752 static storage for braced initializers).

## Macro gaps

kiln's `__cpp_*` set under `-std=c++26` is clang 19's, plus the P0847, P1061 and P2893 macros, `__cpp_constexpr` 202406L (P2747) and `__cpp_constexpr_exceptions` 202411L (P3068, which clang 22 does not define) `__cpp_modules` 1 from C++20 (clang 22's value; clang 19 does not define it) and `__cpp_expansion_statements` 202506L (P1306, which clang 22 does not define), so it differs from clang 22 as follows:

| Macro | kiln (= clang 19) | clang 22 | Row | Action |
|---|---|---|---|---|
| `__cpp_trivial_relocatability` | — | 202502L | P2786 (partial) | None: P2786R13 was removed from C++26 at Kona (Nov 2025), so kiln stays at clang's value (undefined) and the macro is exempt from the M4 C++26 exit (see the P2786 row). |
| `__cpp_expansion_statements` | 202506L | — | P1306 (done) | None: kiln ships P1306R5 ahead of clang 22 (clang 23 defines 202506L), so it defines the macro at C++26 whatever the `-fclang-compat` (`KILN_FEATURES`); exempt from matching clang 22, as for `__cpp_constexpr_exceptions`. |

kiln no longer advertises a macro without the feature: `__cpp_auto_cast` (P0849),
`__cpp_placeholder_variables` (P2169) and `__cpp_static_assert` 202306L (P2741),
which clang 19's predefines carried, are implemented (WP-356).
