// [dcl.constexpr]/3 in C++17: under -pedantic-errors clang's C++20
// extension warning for a variable defined without initialization in a
// constexpr function is an error. A class type is initialized by its
// default constructor ([dcl.init.general]/7.1), a
// range-for variable from its iterator, a structured binding from its
// initializer, and an extern block-scope declaration defines nothing:
// none of those is diagnosed. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17 -pedantic-errors
//@ strict-diags
//@ diag: 13:33 error "uninitialized variable in a constexpr function is a C++20 extension [-Werror,-Wc++20-extensions]"

constexpr int copy(int i) { int j; j = i; return j; }

struct Ctor { int a; constexpr Ctor() : a(1) {} };
struct Pair { int a, b; };

constexpr int ctor_type() { Ctor c; return c.a; }
static_assert(ctor_type() == 1, "");
constexpr int range_for() {
  int r[2] = {1, 2}, s = 0;
  for (int x : r) s += x;
  return s;
}
static_assert(range_for() == 3, "");
constexpr int binding() { auto [x, y] = Pair{1, 2}; return x + y; }
static_assert(binding() == 3, "");
constexpr bool declares() { extern int e; return true; }
static_assert(declares(), "");
