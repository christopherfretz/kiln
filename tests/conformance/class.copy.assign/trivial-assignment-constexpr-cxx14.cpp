// C++14 [class.copy.assign]/10 (N3652): a trivial copy or move
// assignment operator of a literal class is constexpr and is evaluated
// as a copy; before C++14 it is not
// (class.copy.assign/trivial-assignment-not-constexpr-cxx11.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++14

struct T {
  int x = 0;
  bool y = false;
  constexpr T() {}
  constexpr T(int v) : x(v) {}
};
union U { int a; float b; constexpr U() : a(1) {} };

constexpr T t = (T{} = T{4});
static_assert(t.x == 4, "");
constexpr int g(T a, T b) { return (a = b).x; }
static_assert(g(T(1), T(2)) == 2, "");
constexpr int h(U a, U b) { return (a = b).a; }
static_assert(h(U(), U()) == 1, "");
