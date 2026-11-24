// [temp.inst]/3, [dcl.attr.grammar]/5: a function template
// specialization's parameters are those of the template's latest
// declaration, attributes included: written only on the definition
// (`f`), only on a later declaration (`g`), on both (`h`, one), and
// inherited by the definition from the first declaration (`k`, shown
// `Inherited` on the specialization's parameter as on the definition's;
// the first declaration's `align_value` and `maybe_unused` are not
// inherited, so `k<int>` has neither); `k<int>`'s parameters take the
// definition's names. C++23: P3475R2 removes `carries_dependency` from
// C++26. spec/sema.md §4.5 "Parameter attributes" rule 5, WP-3407.
//@ expect: ast
//@ std: c++23
//@ skip(clang): the clang-ast normalizer does not map declaration attributes yet (checked by hand against clang 19 and 22 `-ast-dump`)
template <class T> void f(T *p);
template <class T> void f(T *p __attribute__((nonnull))) {}
template void f<int>(int *);
template <class T> void g(T *p) {}
template <class T> void g(T *p __attribute__((nonnull)));
template void g<int>(int *);
template <class T> void h(T *p __attribute__((nonnull)));
template <class T> void h(T *p __attribute__((nonnull))) {}
template void h<int>(int *);
template <class T>
void k(T *a [[carries_dependency]], T *b __attribute__((nonnull)), T *c [[maybe_unused]],
       T *d __attribute__((align_value(16))));
template <class T> void k(T *w, T *x, T *y, T *z) {}
template void k<int>(int *, int *, int *, int *);
