// [basic.scope.scope], [dcl.attr.grammar]/5: a block-scope function
// declaration's parameter attributes. A later declaration inherits from
// the declaration its lookup found (clang `mergeParamDeclAttributes` from
// the `Old` of `MergeFunctionDecl`): the namespace-scope definition of
// `a4` finds the earlier namespace-scope declaration, not the
// block-scope one, so it has no `nonnull`; `a5`'s finds the block-scope
// declaration and inherits its `nonnull` and `annotate`. A call checks
// `nonnull` (UBSan) from the parameters of the declaration it names:
// `b1` through its block-scope declaration only, `c1` also through a
// block-scope declaration that inherited it. spec/sema.md §4.5
// "Parameter attributes", WP-2476.
//@ expect: ir
//@ args: -fsanitize=nonnull-attribute -Wno-nonnull
void a4(int *p, int *q);
void use4() { void a4(int *p __attribute__((nonnull)), int *q __attribute__((align_value(16)))); }
void a4(int *p, int *q) { *p = *q; }

void use5() { void a5(int *p __attribute__((nonnull)), int *q __attribute__((annotate("pd")))); }
void a5(int *p, int *q) { *p = *q; }

void b1(int *p);
void call1() { void b1(int *p __attribute__((nonnull))); b1(0); }
void call2() { b1(0); }

void c1(int *p __attribute__((nonnull)));
void call3() { void c1(int *p); c1(0); }
