// The parameter attributes of function template specializations whose
// template is declared more than once (template-redecl-ir.cpp,
// template-redecl-O1-ir.cpp; spec/sema.md §4.5 "Parameter attributes"
// rule 5, WP-3407).

// Written only on the definition.
template <class T> void def_only(T *p);
template <class T> void def_only(T *p __attribute__((nonnull))) { *p = 0; }
template void def_only<int>(int *);

// Written only on a later declaration that is not a definition.
template <class T> void redecl_only(T *p) { *p = 0; }
template <class T> void redecl_only(T *p __attribute__((nonnull)));
template void redecl_only<int>(int *);

// Written on both: one attribute.
template <class T> void both(T *p __attribute__((nonnull)));
template <class T> void both(T *p __attribute__((nonnull))) { *p = 0; }
template void both<int>(int *);

// Inherited from the first declaration by the definition.
template <class T> void first_only(T *p __attribute__((nonnull)));
template <class T> void first_only(T *p) { *p = 0; }
template void first_only<int>(int *);

// A declaration after the point where the specialization is declared
// does not change it: an implicit instantiation, an explicit one, and a
// definition after the call.
template <class T> void after_use(T *p) { *p = 0; }
void use_after_use(int *q) { after_use(q); }
template <class T> void after_use(T *p __attribute__((nonnull)));

template <class T> void after_inst(T *p) { *p = 0; }
template void after_inst<int>(int *);
template <class T> void after_inst(T *p __attribute__((nonnull)));

template <class T> void def_after_use(T *p);
void use_def_after_use(int *q) { def_after_use(q); }
template <class T> void def_after_use(T *p __attribute__((nonnull))) { *p = 0; }

// An explicit specialization keeps its own parameters.
template <class T> void spec(T *p);
template <class T> void spec(T *p __attribute__((nonnull))) { *p = 0; }
template <> void spec<long>(long *p) { *p = 0; }
template void spec<int>(int *);

// A member of a class template is instantiated from its declaration in
// the class: the out-of-class definition's attributes do not reach it.
template <class T> struct S {
  void m(T *p);
  template <class U> void t(T *p, U *u);
};
template <class T> void S<T>::m(T *p __attribute__((nonnull))) { *p = 0; }
template <class T>
template <class U>
void S<T>::t(T *p __attribute__((nonnull)), U *u __attribute__((nonnull))) {
  *p = 0;
  *u = 0;
}
template struct S<int>;
template void S<int>::t<long>(int *, long *);

// A member template of a class and a namespace member defined out of
// line: the definition's.
struct C {
  template <class U> void t(U *u);
};
template <class U> void C::t(U *u __attribute__((nonnull))) { *u = 0; }
template void C::t<long>(long *);

namespace N {
template <class T> void f(T *p);
}
template <class T> void N::f(T *p __attribute__((nonnull))) { *p = 0; }
template void N::f<int>(int *);

// A friend declaration is not the declaration lookup finds.
template <class T> void fr(T *p) { *p = 0; }
struct X {
  template <class T> friend void fr(T *p __attribute__((nonnull)));
};
template void fr<int>(int *);

// The other parameter attributes, on the definition only (`noescape`
// on both: it is part of the function type).
template <class T> T *other(T *p __attribute__((noescape)), T *q, T *r, T *s, int u, int v);
template <class T>
T *other(T *p __attribute__((noescape)), T *q __attribute__((align_value(16))),
         T *r __attribute__((annotate("pa"))), T *s __attribute__((nonnull)),
         int u [[maybe_unused]], int v __attribute__((maybe_undef))) {
  return *p + *q + *r ? s : nullptr;
}
int *use_other(int *a, int b) { return other(a, a, a, a, b, b); }

// Not inherited (`align_value`, `annotate` is but shows the same): the
// first declaration's do not reach a specialization of the definition.
template <class T>
void first_not_inherited(T *q __attribute__((align_value(16))), int v __attribute__((maybe_undef)));
template <class T> void first_not_inherited(T *q, int v) { *q = v; }
void use_first_not_inherited(int *a, int b) { first_not_inherited(a, b); }
