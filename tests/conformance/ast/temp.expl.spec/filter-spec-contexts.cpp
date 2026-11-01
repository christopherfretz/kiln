//@ expect: ast
//@ ast-filter: C
// [temp.expl.spec], [temp.spec.partial], [temp.mem], [temp.explicit]:
// clang's -ast-dump-filter names a member of a class template
// specialization by the specialization's arguments without the trailing
// ones equal to their defaults (`n::A<char>::B<int>::C`), a member of a
// partial specialization by its canonical arguments
// (`X<type-parameter-0-0 *>::C`), and a member of a template's pattern by
// the template's name (`n::A::B::C`). It searches templates' patterns and
// explicit specializations where they are declared, but not implicit
// instantiations or the members of explicit instantiations.
namespace n {
template <class T, int I = 0>
struct A {
  template <class U> struct B { struct C { T t; U u; }; };
  template <class U> struct B<U *> { struct C { U *pu; }; };
  struct C { int c; };
};
template <class T>
struct A<T, 1> {
  struct C { T x; };
  template <class U> struct B { struct C { U y; }; };
};
template <>
struct A<long, 2> {
  struct C { long z; };
  template <class U> struct B { struct C { U w; }; };
};
}
template <> template <class U> struct n::A<char>::B { struct C { U v; }; };
template <> template <> struct n::A<char>::B<int>::C { int q; };
template <> struct n::A<short>::C { short s; };
template struct n::A<float>;
extern template struct n::A<double, 3>;
n::A<int>::B<int>::C use1;

template <class T, bool B = false, class U = T> struct X { struct C { int x; }; };
template <class T> struct X<T *> { struct C { int p; }; };
template <class T> struct X<T &, true> { struct C { int r; }; };
template <class T> struct X<T[], false, int> { struct C { int a; }; };
template <> struct X<int>::C { int i; };
template <> struct X<int, false, long>::C { int j; };

template <template <class> class TT, class T> struct D { struct C { TT<T> m; }; };
template <class T> struct E {};
namespace q { template <class T> struct F {}; }
template <> struct D<E, int> { struct C { int e; }; };
template <> struct D<q::F, int>::C { int f; };
