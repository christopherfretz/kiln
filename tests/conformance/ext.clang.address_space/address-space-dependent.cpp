//@ expect: compile-ok
//@ strict-diags
//@ xfail(kiln): WP-1803 follow-up: a value-dependent address_space(N) (clang DependentAddressSpaceType) is rejected as not an integer constant
// A template's address_space(N) whose N depends on a template parameter
// takes its space at instantiation (clang DependentAddressSpaceType).

template <int N> struct holder {
  using type = int __attribute__((address_space(N)));
};
template <class A, class B> constexpr bool same_v = false;
template <class A> constexpr bool same_v<A, A> = true;
static_assert(same_v<holder<1>::type, __attribute__((address_space(1))) int>);
static_assert(!same_v<holder<2>::type, holder<1>::type>);
template <int N> int read(__attribute__((address_space(N))) int *p) { return *p; }
int use(__attribute__((address_space(3))) int *p) { return read<3>(p); }
