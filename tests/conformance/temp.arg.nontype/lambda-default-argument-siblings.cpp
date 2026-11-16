// [temp.param]/15, [expr.prim.lambda.closure]/1: a lambda as the default
// argument of a template parameter, in class, function and alias
// templates at namespace scope and as members of a class template, with
// a class NTTP `N` used inside it; each use of the default gets its own
// closure type. Found running GCC's C++ testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
struct X { int n; };
template <class T> struct A {
  // class template member
  template <auto N, auto F = [] { return N.n; }> struct C { static constexpr int v = F(); };
  // function template member
  template <auto N, auto F = [] { return N.n * 2; }> static constexpr int f() { return F(); }
  // variable template member
  template <auto N, auto F = [](auto y) { return N.n + y; }> static constexpr auto v = F;
  // alias template member
  template <auto N, auto F = [] { return N; }> using R = decltype(F());
  // each use of the default is a new closure type
  template <auto N, auto F = [] {}> struct K { using type = decltype(F); };
};
template <auto N, auto F = [] { return N.n; }> struct NC { static constexpr int v = F(); };
template <auto N, auto F = [] { return N.n * 2; }> constexpr int nf() { return F(); }
template <auto N, auto F = [] { return N; }> using NR = decltype(F());
template <auto N, auto F = [] {}> struct NK { using type = decltype(F); };

static_assert(A<int>::C<X{3}>::v == 3);
static_assert(A<int>::f<X{4}>() == 8);
static_assert(A<int>::v<X{5}>(1) == 6);
static_assert(__is_same(A<int>::R<X{6}>, X));
static_assert(!__is_same(A<int>::K<1>::type, A<int>::K<2>::type));
static_assert(NC<X{3}>::v == 3);
static_assert(nf<X{4}>() == 8);
static_assert(__is_same(NR<X{6}>, X));
static_assert(!__is_same(NK<1>::type, NK<2>::type));

template <class T> struct D {
  template <class U> struct E {
    template <auto N, auto F = [](auto y) { return decltype(N){N.n + y}; }>
    static constexpr auto w = F;
    template <class V> static constexpr int g() { return w<X{1}>(V{41}).n; }
  };
};
static_assert(D<int>::E<int>::g<int>() == 42);
