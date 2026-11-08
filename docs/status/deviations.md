# Deliberate deviations from the oracle

Behaviours where kiln follows the standard rather than clang (the oracle,
docs/VALIDATION.md), because clang's behaviour is a defect rather than a
choice. Each row names the rule kiln implements, what clang (and GCC)
do, the spec paragraph that states it, and the corpus test that pins it
(skipped under clang with `skip(clang)`, or pinned to a release with
`requires:`, spec/harness.md). Smaller, codegen-only differences are
listed where they live (spec/codegen.md "Deviation" paragraphs).

| Rule | kiln | clang / GCC | Spec | Test |
|---|---|---|---|---|
| [temp.point]/7, [temp.inst]/7: the end of the TU is a point of instantiation of a variable template specialization or static data member usable in constant expressions that is odr-used only before its template's definition (WP-490) | defined from the later definition at the end of the TU, no warning | clang instantiates such a variable only at a reference (`DoMarkVarDeclReferenced` never queues it) and leaves it undefined; GCC 14/16 too; both fail to link | spec/sema-templates.md §4.6 "Initializers" | tests/conformance/temp.point/var-template-end-of-tu-instantiation-run.cpp |
| [GNU] compound literal (C99 6.5.2.5p5), [temp.inst]: a literal in a constexpr variable template or a class template's default member initializer is at file scope wherever the specialization is instantiated from (WP-490) | a file-scope object of static storage duration at every compat level, as clang 19 and GCC 16 | clang 21+ (llvm-project 977d8a4bcd83, `Scope::isInCFunctionScope` on the parser's scope) makes it a block-scope temporary when it is instantiated from a function body: clang 22 warns `-Wdangling` and rejects the constexpr initialization | spec/sema.md §5.8 "Compound literals" Deviations | tests/conformance/ext.gnu.compound.literal/variable-template-from-function-run.cpp |

## GCC 16.1 reflection divergences

Behaviours where kiln's reflection follows the IS rather than GCC 16.1,
the reflection oracle (`oracle: gcc`, spec/reflection.md §1.2). Each was
triaged when the GCC differential found it (docs/plans/m6-reflection.md
§5, exit measure (e)).

| Rule | kiln | clang / GCC | Spec | Test |
|---|---|---|---|---|
| [meta.reflection.traits]/3.2: a [meta.reflection.traits] function whose associated trait specialization would have undefined behavior because of an incomplete type ([meta.rqmts]/5, [res.on.functions]/2.5; the Preconditions column of [meta.unary.prop] for `is_copy_constructible` asks for "a complete type, cv void, or an array of unknown bound") is not a constant subexpression: `std::meta::is_copy_constructible_type(^^T)`, and the other type-trait functions, with `T` an incomplete class type | the call is not a constant expression, with the note "invalid use of incomplete type 'T'"; kept by the operator's decision (2026-11-05) | clang has no reflection. GCC 16.1 returns a value: `false` for `is_copy_constructible_type(^^T)` and `is_constructible_type(^^T, {})`, `true` for `is_trivially_copyable_type(^^T)`; it diagnoses `is_empty_type(^^Tmpl<int>)` as kiln does | spec/reflection-traits.md §2.2 | tests/conformance/meta.reflection.traits/failures-neg.cpp |
| [temp.variadic]/5.4: the pattern of a base-specifier pack expansion is the whole base-specifier, so an annotation operand naming the pack (`[[=sizeof(Bs)]] Bs...`) is substituted per base, and one on a base-specifier that is not an expansion is the unexpanded-pack error | kiln: one annotation per base with that base's value; the error at the template definition | GCC 16.1: internal compiler error in `dependent_type_p` at instantiation (both forms); no error at definition | spec/annotations.md | `meta.reflection.annotation/base-specifier-pack-operand.cpp`, `base-specifier-pack-operand-neg.cpp` (WP-867) |
