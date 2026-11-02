// Mangling differential: the requires-clause of a template template
// parameter's head (`Tt <param-decls> Q <constraint> E`, spec/abi.md
// §8.16, [temp.param]/1, [temp.arg.template], WP-314). Template
// parameters in the clause keep their real depth (`TL<d-1>_<i-1>_`): the
// head's own parameters are one level below the template that declares
// `TT`, an earlier parameter of that template is at its depth, and
// parameters of enclosing class templates and of a `Tt` nested inside
// the head are at theirs.

template <class T> concept C = sizeof(T) >= 1;
template <class T, class U> concept D = sizeof(T) == sizeof(U);
template <class T> struct Z {};
template <class T, class U> struct Z2 {};
template <template <class> class> struct ZZ {};

// The clause names an earlier parameter of the same template-head.
template <class X, template <class U> requires D<U, X> class TT> void t1() {}

// The clause names an enclosing class template's parameter.
template <class X> struct S {
  template <template <class U> requires D<U, X> class TT> void m1() {}
  template <template <class U> requires C<U> class TT> void m2() {}
  template <class Y> struct I {
    template <template <class U> requires D<U, X> && D<U, Y> class TT> void m3() {}
  };
};

// A `Tt` nested in a `Tt`, each with a clause.
template <template <template <class V> requires C<V> class W> requires C<W<int>> class TT>
void t2() {}

// Two parameters, and a pack in a fold.
template <template <class U, class V> requires D<U, V> class TT> void t3() {}
template <template <class... U> requires (C<U> && ...) class TT> void t4() {}

void use() {
  t1<int, Z>();
  S<int>().m1<Z>();
  S<int>().m2<Z>();
  S<int>::I<char>().m3<Z>();
  t2<ZZ>();
  t3<Z2>();
  t4<Z>();
}
