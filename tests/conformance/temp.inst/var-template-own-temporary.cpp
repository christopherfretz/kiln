// [temp.inst]/7, [class.temporary]/6: an instantiated variable template
// specialization whose non-dependent initializer binds a temporary binds
// a temporary of its own (two specializations have distinct ones), and
// an out-of-class definition of a static constexpr array member of a
// class template takes its dependent bound from the in-class declaration
// ([dcl.array]/8). Found running GCC's C++ testsuite (WP-1468).
//@ std: c++17
//@ expect: compile-ok
template <int N> struct C { static constexpr int a[N] = { N, N + 1 }; };
template <int N> constexpr int C<N>::a[];
static_assert(C<2>::a[1] == 3);
template <class> constexpr const int &v = 1;
static_assert(v<int> == 1);
static_assert(&v<int> != &v<char>);
template <class T> struct S { template <class> static constexpr const T &w = T(2); };
static_assert(S<int>::w<int> == 2);
