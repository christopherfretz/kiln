# libc++ 19 sample long tail (WP-276)

WP-276 reduced the 42 long-tail failures of the 10% libc++ 19 sample (2026-11-01, 555/790
passing; see `docs/status/libcxx.md`) to standalone reproducers and classified each by cause.
The small ones were fixed on `wp/276`. Every reproducer is a corpus test of at most 40 lines that
passes under kiln and under `--compiler clang++-19`. A reproducer that is still failing carries
`xfail(kiln): <cause>`.

Rerun of the 42 tests on `wp/276` (`cargo xtask libcxx --no-fail -j 4 <tests>`): **31 pass, 11 fail**.
The 11 failures are lazy_split 15–18, `<expected>` 25–30 and inout_ptr 36. All three groups are listed under
"Belongs to other work packages" below.

Cause classes:

- **sema**: wrong acceptance, rejection or semantics in `kiln_sema`.
- **oven**: the constant evaluator (`kiln_oven`).
- **codegen**: IR emission or linking (`kiln_codegen`).
- **parser**: `kiln_parse`.

Status values:

- **fixed**: the reproducer passes with the fix on this branch.
- **xfail(reason)**: the reproducer is committed with an xfail.
- **passes now**: fixed elsewhere, nothing to reproduce.
- **belongs to WP-NNN**: the cause is in another package's cluster. The reduction is inline below.

Paths are relative to `tests/conformance/`. Test paths are relative to `libcxx/test/std/`.

| # | test | first diagnostic | cause class | reproducer | status |
|---|------|------------------|-------------|------------|--------|
| 1 | algorithms/…/alg.copy/copy_backward.pass.cpp | static assertion expression is not an integral constant expression | oven: a trivial copy wrote the tail padding, not the dsize | class.copy.assign/trivial-copy-base-tail-padding.cpp | fixed |
| 2 | algorithms/…/alg.foreach/pstl.for_each.pass.cpp | IR TypeMismatch "call argument 1: expected i1, found ptr" | codegen: no load through the lvalue `UserDefinedConversion` of `operator bool&()` | class.conv.fct/reference-returning-conversion-to-value-run.cpp | fixed |
| 3 | concepts/…/relation.subsumption.compile.pass.cpp | call to 'check_relation_subsumes_itself' is ambiguous | sema: the concept-id parameter mapping did not group the trailing pack arguments | temp.constr.constr/concept-pack-parameter-mapping-subsumption.cpp | fixed |
| 4 | containers/…/map.access/index_key.pass.cpp | call to constructor of 'InAllocatorConstructGuard' is ambiguous | sema: an initializer of a dependent class type was converted at definition; `T::name<` through a typedef specialization | temp.dep.type/current-instantiation-ctor-init.cpp, temp.names/typedef-qualified-member-template-uninstantiated.cpp | fixed |
| 5 | containers/…/map.modifiers/insert_node_type_hint.pass.cpp | variable 'nf' with type 'auto *' has incompatible initializer of type '\<overloaded function type\>' | sema: an uncalled dependent template-id was resolved at definition | over.over/template-id-in-template.cpp | fixed |
| 6 | containers/…/multimap.modifiers/extract_iterator.pass.cpp | returning reference to local temporary object | sema: a reference member access was treated as extending (and dangling) the temporary | class.temporary/reference-member-access.cpp | fixed |
| 7 | containers/…/list.cons/size_value_alloc.pass.cpp | runtime `std::bad_array_new_length` | codegen (overflow builtins) | none | passes now (on main after the 98037fad merge; most likely WP-270's overflow builtins; not bisected) |
| 8 | containers/…/list.modifiers/insert_iter_value.pass.cpp | runtime `std::bad_array_new_length` | codegen (overflow builtins) | none | passes now (as #7) |
| 9 | containers/…/vector.cons/construct_iter_iter_alloc.pass.cpp | as #4 | sema | as #4 | fixed |
| 10 | containers/…/unord.map.elem/index.pass.cpp | as #4 | sema | as #4 | fixed |
| 11 | containers/…/unord.map.modifiers/insert_node_type.pass.cpp | as #5 | sema | as #5 | fixed |
| 12 | containers/…/unord.multimap.modifiers/insert_node_type_hint.pass.cpp | as #5 | sema | as #5 | fixed |
| 13 | language.support/support.types/nullptr_t.pass.cpp | static assertion failed due to requirement '!has_less\<std::nullptr_t\>::value' | sema: relational operators accepted `std::nullptr_t` | expr.rel/nullptr-operand.cpp | fixed |
| 14 | numerics/…/valarray.transcend/pow_valarray_valarray.pass.cpp | link: undefined `__cxa_allocate_exception.1` | codegen: a user `extern "C"` redeclaration of a runtime function got a renamed `.1` copy | dcl.link/extern-c-runtime-function-redeclaration.cpp | fixed |
| 15 | ranges/…/range.lazy.split/constraints.compile.pass.cpp | static assertion failed due to requirement '!...' (types.h:114) | sema: a call returning `const X` was not a const prvalue; constrained member partial specializations; a member class of a member template specialization. Remaining: friend-template access | expr.call/const-class-prvalue-result.cpp, temp.spec.partial.member/constrained-member-partial-match.cpp, temp.mem.class/nested-class-of-member-template-names-sibling.cpp | partly fixed; remaining belongs to WP-278 |
| 16 | ranges/…/range.lazy.split.inner/ctor.outer_iterator.pass.cpp | as #15 | sema | as #15 | as #15 |
| 17 | ranges/…/range.lazy.split.outer.value/end.pass.cpp | as #15 | sema | as #15 | as #15 |
| 18 | ranges/…/range.lazy.split/view_interface.pass.cpp | as #15 | sema | as #15 | as #15 |
| 19 | re/re.submatch/re.submatch.op/compare.pass.cpp | aggregate expression CXXRewrittenBinaryOperator is not supported yet | codegen: aggregate `CXXRewrittenBinaryOperator`; sema: reversed `<=>` was not `0 <=> (y <=> x)` | over.match.oper/reversed-spaceship-run.cpp | fixed |
| 20 | thread/futures/futures.shared_future/wait_for.pass.cpp | as #14 | codegen | as #14 | fixed |
| 21 | thread/futures/…/futures.task.nonmembers/swap.pass.cpp | allocating an object of abstract class type '_FF' | sema: an operator function did not override a virtual base operator function | class.virtual/operator-function-override.cpp | fixed |
| 22 | thread/futures/futures.unique_future/wait.pass.cpp | as #14 | codegen | as #14 | fixed |
| 23 | thread/…/condvarany/wait_until_token_pred.pass.cpp | runtime "condition_variable::timed wait: mutex not locked" | sema: in a template, `T x(e)` / `T x{e}` with `e` of class `T` was instantiated as a default-initialization (silent miscompile) | temp.inst/direct-init-from-same-class-run.cpp | fixed |
| 24 | thread/…/sharedtimedmutex.class/try_lock_until.pass.cpp | link: undefined `__shared_mutex_base::__write_entered_` | codegen: a const static member read through `b.w` was not folded (not an odr-use) | basic.def.odr/static-member-via-member-access-not-odr-used.cpp | fixed |
| 25 | utilities/expected/…/ctor/ctor.convert.copy.pass.cpp | static assertion expression is not an integral constant expression | sema: no prospective destructor selection ([class.dtor]/4): the constrained `= default` destructor is not chosen, so the class is not trivially destructible | class.dtor/prospective-destructor-selection.cpp (temp.pre/requires-clause-builtin-trait.cpp was found on the way and fixed) | xfail(kiln: prospective destructor selection, [class.dtor]/4); proposed new WP |
| 26 | utilities/expected/…/ctor/ctor.unexpected.copy.pass.cpp | as #25 | sema | as #25 | as #25 |
| 27 | utilities/expected/…/observers/arrow.pass.cpp | as #25 | sema | as #25 | as #25 |
| 28 | utilities/expected/expected.void/ctor/ctor.convert.copy.pass.cpp | as #25 | sema | as #25 | as #25 |
| 29 | utilities/expected/expected.void/dtor.pass.cpp | static assertion failed due to requirement 'is_trivially_destructible_v' | sema | as #25 | as #25 |
| 30 | utilities/expected/expected.void/observers/error_or.pass.cpp | as #25 | sema | as #25 | as #25 |
| 31 | utilities/…/func.bind.place/placeholders.pass.cpp | static assertion expression is not an integral constant expression | oven: a trivial copy of an empty class read its (uninitialized `extern const`) source | expr.const/trivial-copy-empty-class-no-read.cpp | fixed |
| 32 | utilities/…/func.wrap.func.con/F_incomplete.pass.cpp | allocating an object of abstract class type '_Fun' | sema | as #21 | fixed |
| 33 | utilities/…/func.wrap.func.inv/invoke.pass.cpp | as #32 | sema | as #21 | fixed |
| 34 | utilities/…/util.smartptr.shared.cmp/cmp_nullptr.pass.cpp | as #19 | codegen + sema | as #19 | fixed |
| 35 | utilities/optional/…/explicit_optional_U.pass.cpp | static assertion expression is not an integral constant expression | oven: a constexpr destructor destroyed an anonymous union member | class.dtor/anonymous-union-member-constexpr.cpp | fixed |
| 36 | utilities/smartptr/adapt/inout_ptr/inout_ptr_t.ctor.pass.cpp | kiln: not yet implemented: pack expansion in this context | sema: an expansion mixing an outer pack and a lambda's pack (`templ/pack.rs:135`) | inline below | belongs to WP-282 |
| 37 | utilities/…/unique.ptr.special/cmp_nullptr.pass.cpp | as #19 | codegen + sema | as #19 | fixed |
| 38 | utilities/template.bitset/bitset.members/any.pass.cpp | '__storage_type' is a private member of '__bit_iterator\<…\>' | sema: a non-dependent non-type argument of a dependent specialization (`It<C, false>`) did not compare by profile, so the definition was a new template | temp.over.link/non-type-literal-arg-in-dependent-specialization.cpp | fixed |
| 39 | utilities/template.bitset/bitset.members/not_all.pass.cpp | as #38 | sema | as #38 | fixed |
| 40 | utilities/template.bitset/bitset.members/set_all.pass.cpp | as #38 | sema | as #38 | fixed |
| 41 | utilities/template.bitset/bitset.operators/op_not.pass.cpp | as #38 | sema | as #38 | fixed |
| 42 | utilities/tuple/tuple.tuple/TupleFunction.pass.cpp | as #32 | sema | as #21 | fixed |

## Summary by cause class

| class | tests | fixed | remaining |
|-------|-------|-------|-----------|
| sema | 3, 4, 5, 6, 9–13, 15–18, 21, 23, 25–30, 32, 33, 36, 38–42 | 21 | lazy_split 15–18 (WP-278), prospective destructors 25–30 (new WP), inout_ptr 36 (WP-282) |
| oven | 1, 31, 35 | 3 | none |
| codegen | 2, 14, 19, 20, 22, 24, 34, 37 (7, 8 fixed on main) | 8 | none |
| parser | none (the requires-clause trait parse was found while reducing 25–30) | | |

## Belongs to other work packages

**WP-278 (friend-template access).** For lazy_split 15–18 the remaining errors are at
lazy_split_view.h:331, where a member template friend of a member class template is not
honoured, and at :394, the hidden-friend `iter_swap`'s `noexcept`. Reduction:

```cpp
template <class V> struct view {
  template <bool> struct inner;
  template <bool C> struct outer {
  private:
    template <bool> friend struct inner;
    int cur() const { return 1; }
  };
  template <bool C> struct inner { outer<C> o; int get() const { return o.cur(); } };
};
int x = view<int>::inner<true>{}.get();
```

**WP-282 (`templ/pack.rs:135`).** For inout_ptr 36, an expansion mixes the enclosing pack `Args`
with a generic lambda's pack `xs`. It needs a `SubstTemplateTypeParmPack`-like substitution of the
outer pack inside the lambda. Reduction:

```cpp
template <class... A> int sum(A... a) { return (0 + ... + a); }
template <class... Args> struct S { int run() {
  auto f = [&](auto&&... xs) { return sum(static_cast<Args>(xs)...); };
  return f(Args()...); } };
int x = S<int, long>{}.run();
```

**Proposed: prospective destructor selection ([class.dtor]/4).** This blocks 25–30. At the class's
closing `}`, and again at instantiation, kiln must choose the destructor among the prospective
destructors by their constraints. `Ast::destructor` (kiln_ast/src/ast.rs) would consult a side
table that records the selected destructor. The implicit-destructor walk in
kiln_sema/src/class/special/implicit.rs would be restricted to it. Estimated at 200–400 lines.
`class.dtor/prospective-destructor-selection.cpp` is the xfail reproducer.
