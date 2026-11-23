// [dcl.attr.grammar]/5, [basic.scope.scope]: a parameter attribute clang
// keeps as an `InheritableParamAttr` (`nonnull`, `annotate`) is inherited
// by the corresponding parameter of a later declaration (clang
// `mergeParamDeclAttributes`), so a definition gets it from a prior
// declaration: its `nonnull` and its `llvm.var.annotation`. `align_value`
// is not inherited: only the definition's own sets `align`. Three
// placements: the prior declaration only, the definition only, both
// (the definition's `align_value` wins). spec/sema.md §4.5 "Parameter
// attributes", WP-2476.
//@ expect: ir
void a1(int *p __attribute__((nonnull)), int *q __attribute__((align_value(16))),
        int *r __attribute__((annotate("pa"))));
void a1(int *p, int *q, int *r) { *p = *q + *r; }

void a2(int *p, int *q, int *r);
void a2(int *p __attribute__((nonnull)), int *q __attribute__((align_value(16))),
        int *r __attribute__((annotate("pb")))) { *p = *q + *r; }

void a3(int *p __attribute__((nonnull)), int *q __attribute__((align_value(16))),
        int *r __attribute__((annotate("pc"))));
void a3(int *p __attribute__((nonnull)), int *q __attribute__((align_value(32))),
        int *r __attribute__((annotate("pc")))) { *p = *q + *r; }
