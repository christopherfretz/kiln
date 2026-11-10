// Mangling differential (Itanium C++ ABI 5.1.5, [temp.dep.type]/1;
// WP-1169): a qualifier that names the current instantiation, as the
// injected-class-name `C::` or as `C<X>::`, is mangled by clang's
// `Type::Record` case as the bare class name: `sr1CE1f` in a member of
// `C<int>`, not `sr1CIiEE1f`. A qualifier written as some other
// specialization keeps its arguments (`sr1BIiEE1f`, member_qualifier.cpp).
namespace ci {
template <int N> struct I {};
template <class> struct B { template <class U> static constexpr int f() { return 1; } };
template <class X> struct C {
  template <class U> static constexpr int f() { return sizeof(U) + sizeof(X); }
  static constexpr int N = sizeof(X);
  template <class U> I<C::template f<U>() + N> g(U) { return {}; }
  template <class U> I<C<X>::template f<U>() + N> k(U) { return {}; }
  template <class U> I<B<X>::template f<U>()> m(U) { return {}; }
};
void use(C<int> &c) { c.g('a'); c.k('a'); c.m('a'); }
} // namespace ci
