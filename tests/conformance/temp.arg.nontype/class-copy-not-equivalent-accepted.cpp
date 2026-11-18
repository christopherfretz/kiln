// [temp.arg.nontype]/4: a class-type argument A introduces
// `constexpr T v = A;`, the template parameter object is copy-initialized
// from v, and a copy that is not template-argument-equivalent to v makes
// the program ill-formed. clang 22 and GCC 16.1 accept such a copy, and
// so does kiln (docs/status/deviations.md): the parameter object of
// `get<kk>` holds the copy constructor's value, and a prvalue argument
// initializes it directly. A copy that would point into the temporary is
// rejected by all three, if with other wording. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ args: -std=c++20

struct K {
  int x;
  constexpr K(int v) : x(v) {}
  constexpr K(const K &o) : x(o.x + 1) {}
};
template <K n> constexpr int get() { return n.x; }
constexpr K kk(1);
static_assert(get<kk>() == 2, "copied from kk");
static_assert(get<K(1)>() == 1, "initialized from the prvalue");
template <K n> struct T { static constexpr int v = n.x; };
static_assert(T<kk>::v == 2, "");
