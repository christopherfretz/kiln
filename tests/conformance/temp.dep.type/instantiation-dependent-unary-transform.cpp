// [temp.dep.type]/7, [meta.trans.ref]: `TC<N>*` with `N` a constant local of
// a function template is instantiation-dependent but not dependent, so a
// builtin type transform over it is evaluated (clang `BuildUnaryTransformType`
// tests `isDependentType`). Clang 22 substitutes an alias template with its
// arguments as written, so `rr<TC<N>*>` reaches `__remove_reference_t` with
// the sugared argument; the constraint on `S` must still see `TC<3> *`
// (libc++ 22 `subrange(from, to)` in the range insertion tests).
//@ expect: compile-ok
template <int N> struct TC {};
template <class T> using rr = __remove_reference_t(T);
template <class T> concept eq = requires(const rr<T>& t) { t == t; };
template <class I> requires eq<I> struct S { S(I) {} };
template <class I> requires eq<I> struct D { D(I, I) {} };

template <class X> void g() {
  constexpr int N = 3;
  using T = TC<N>;
  S<T*> s(nullptr);
  T* p = nullptr;
  D d(p, p);
  static_assert(__is_same(rr<T*&>, TC<3>*));
  static_assert(__is_same(__add_pointer(T), TC<3>*));
  (void)s;
  (void)d;
}
template void g<int>();
