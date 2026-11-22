// [clang ext] [temp.inst] A type written in an address space over a
// template parameter, or over a value-dependent `address_space(N)` type,
// whose substituted type is already in a different space is clang's
// "conflicting address space qualifiers are provided between types"
// (`TreeTransform::RebuildQualifiedType`), at the type as written; an
// alias template's at its `using` (its type is substituted without
// source locations). The same space again is accepted. spec/sema.md
// "Address spaces", WP-1914.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:20 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 30:1 note "in instantiation of template type alias 't' requested here"
//@ diag: 33:47 error "conflicting address space qualifiers are provided between types 'int __attribute__((address_space(1))) __attribute__((address_space(N)))' and '__attribute__((address_space(2))) int'"
//@ diag: 35:1 note "in instantiation of template class 'S<2>' requested here"
//@ diag: 38:50 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 45:42 note "in instantiation of template class 'X<__attribute__((address_space(2))) int>' requested here"
//@ diag: 39:16 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 40:20 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 41:10 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 42:13 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 43:3 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 48:38 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int[3]'"
//@ diag: 49:45 note "in instantiation of template class 'A<__attribute__((address_space(2))) int[3]>' requested here"
//@ diag: 29:20 error "conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
//@ diag: 50:31 note "in instantiation of template type alias 't' requested here"
//@ diag: 51:42 note "in instantiation of template class 'W<__attribute__((address_space(2))) int>' requested here"
//@ diag: 55:3 error "no matching function for call to 'f'"
//@ diag: 52:25 note "candidate template ignored: substitution failure [with T = __attribute__((address_space(2))) int]: conflicting address space qualifiers are provided between types '__attribute__((address_space(1))) T' and '__attribute__((address_space(2))) int'"
template <class T> using t = T __attribute__((address_space(1)));
t<__attribute__((address_space(2))) int> *p1;
t<__attribute__((address_space(1))) int> *p2;
template <int N> struct S {
  using u = __attribute__((address_space(N))) int __attribute__((address_space(1)));
};
S<2>::u *p3;
S<1>::u *p4;
template <class T> struct X {
  static __attribute__((address_space(1))) const T *a;
  static const T __attribute__((address_space(1))) *b;
  static void (*d)(T __attribute__((address_space(1))) *);
  static T __attribute__((address_space(1))) *e[2];
  using U = T __attribute__((address_space(1)));
  T __attribute__((address_space(1))) *m;
};
X<__attribute__((address_space(2))) int> x2;
X<__attribute__((address_space(1))) int> x1;
X<int> x0;
template <class T> struct A { static T __attribute__((address_space(1))) *m; };
A<__attribute__((address_space(2))) int[3]> a2;
template <class T> struct W { t<T> *p; };
W<__attribute__((address_space(2))) int> w2;
template <class T> void f(T __attribute__((address_space(1))) *);
void g() {
  __attribute__((address_space(2))) int *q = nullptr;
  f<__attribute__((address_space(2))) int>(q);
}
