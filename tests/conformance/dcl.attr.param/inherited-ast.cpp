// [dcl.attr.grammar]/5: a redeclaration's parameter inherits the
// previous declaration's `nonnull`, `annotate` and `carries_dependency`
// (clang `mergeParamDeclAttributes`, shown `Inherited`), but not its
// `align_value`, `noescape`, `maybe_undef` or `maybe_unused`; one it has
// itself is not inherited again; a third declaration inherits through
// the second. C++23: P3475R2 removes `carries_dependency` from C++26.
// spec/sema.md §4.5 "Parameter attributes", WP-2476.
//@ expect: ast
//@ std: c++23
//@ skip(clang): the clang-ast normalizer does not map declaration attributes yet (checked by hand against clang 19 and 22 `-ast-dump`)
void f(int *a __attribute__((nonnull)), int *b __attribute__((annotate("x"))),
       int *c [[carries_dependency]], int *d __attribute__((align_value(16))),
       int *e __attribute__((noescape)), int g __attribute__((maybe_undef)), int h [[maybe_unused]]);
void f(int *a __attribute__((nonnull)), int *b, int *c, int *d, int *e __attribute__((noescape)), int g, int h);
void f(int *a, int *b, int *c, int *d, int *e __attribute__((noescape)), int g, int h) {}
