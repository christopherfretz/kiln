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
