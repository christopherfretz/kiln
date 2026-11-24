// [class.temporary]/6, [temp.inst]: the temporaries a static-storage
// declaration of an instantiation extends are that specialization's own
// objects, mangled for the instantiated declaration (`_ZGR<var>_`, then
// `_ZGR<var>0_`, ... in order): a static local reference bound to a
// member of a temporary, the reference members of a static aggregate
// local and an inline static data member. Before WP-3271 they were the
// pattern's objects, such as one internal `_ZGRZ5localvE1r_` shared by
// every specialization. Compared with clang's IR.
//@ expect: ir
//@ std: c++17
struct S { int a[2]; };
struct A { const S &s; const int &i; };
struct W { S s; };
template <class T> const int &local() {
  static const int &r = S{{1, 2}}.a[1];
  return r;
}
template <class T> const A &agg() {
  static A a{S{{3, 4}}, 5};
  return a;
}
template <class T> struct X {
  static inline const A m{W{S{{6, 7}}}.s, 8};
};
int use() {
  return local<int>() + local<long>() + agg<int>().i + agg<long>().s.a[0] +
         X<int>::m.i + X<long>::m.s.a[1];
}
