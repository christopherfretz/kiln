// [temp.dep.type]/6, [temp.res.general]/3: `typename TT<V>::In::x`, a member of
// a member class of a specialization of a template template parameter, is a
// dependent name looked up at instantiation; it names the member type of the
// argument template's specialization (no spurious "no type named 'x'").
//@ expect: compile-ok
template <class T> struct Box { struct In { using x = T; static constexpr int v = 1; template <class U> struct M { using y = U; }; }; };
template <template <class> class TT> struct A {
  using t1 = typename TT<int>::In::x;
  static constexpr int a = TT<int>::In::v;
  using t2 = typename TT<int>::In::template M<char>::y;
  void f() { typename TT<long>::In::x q = 0; (void)q; }
};
template <template <class> class TT, class V> struct B {
  using t = typename TT<V>::In::x;
  static constexpr int b = TT<V>::In::v;
};
A<Box> a; B<Box, int> b;
static_assert(__is_same(A<Box>::t1, int));
static_assert(__is_same(A<Box>::t2, char));
static_assert(__is_same(B<Box, short>::t, short));
template <template <class> class TT> using Al = typename TT<int>::In::x;
static_assert(__is_same(Al<Box>, int));
template <template <class> class TT> typename TT<int>::In::x g();
static_assert(__is_same(decltype(g<Box>()), int));
template <template <class...> class TT> struct C { using t = typename TT<int>::In::x; };
static_assert(__is_same(C<Box>::t, int));
