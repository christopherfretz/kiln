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
