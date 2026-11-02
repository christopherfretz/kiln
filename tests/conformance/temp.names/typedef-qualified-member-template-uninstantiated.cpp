// [temp.names]/3, [basic.lookup.qual.general]/1: `T::name<` where the
// typedef `T` names a class template specialization not yet instantiated
// still finds the member template `name`, so its `<` opens a template
// argument list inside an enclosing one. libc++ test/support/
// container_test_types.h:350 (`A1T::rebind_traits<float>`; map index_key,
// unord.map index, vector construct_iter_iter_alloc, WP-276).
// Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class T, class U> struct W { static const bool value = true; };
template <class A> struct traits {
  template <class T> using rebind = traits<T>;
  template <class T> struct nested { static const int n = 4; };
  template <class T> static constexpr int f() { return 5; }
  template <class T> static constexpr int v = 6;
};
typedef traits<int> A1T;
static_assert(W<A1T::rebind<float>, int>::value, "");
typedef traits<char> A2T;
static_assert(W<A2T::nested<float>, int>::value, "");
typedef traits<long> A3T;
static_assert(A3T::f<int>() + W<A3T::rebind<int>, int>::value == 6, "");
typedef traits<short> A4T;
static_assert(A4T::v<int> == 6 && A4T::nested<int>::n == 4, "");
typedef traits<unsigned> A5T;
bool b = W<int, A5T::rebind<float>>::value;
