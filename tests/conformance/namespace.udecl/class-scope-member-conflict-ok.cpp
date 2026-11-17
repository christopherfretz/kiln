// [namespace.udecl]/11, the accepted side: a class and a data member, or a
// class and a function, do not correspond ([basic.scope.scope]/4), so a
// using-declarator may bring one beside the other in either order; a
// member function hides a base function with the same signature named by
// a using-declarator ([namespace.udecl]/14) and overloads one with another
// signature; `using enum` of another class's enumeration beside unrelated
// members; at block and namespace scope a using-declarator naming an
// enumerator already in scope is the same entity, no conflict (P1787); and
// at namespace scope a class beside a using-declared typedef-name is kept
// (clang 22 and GCC 16 both accept it). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++20
struct H {
  int v;
  void f(int);
  void g(int);
  struct C {};
};
struct J1 : H { struct v {}; using H::v; };
struct J2 : H { using H::v; struct v {}; };
struct J3 : H { struct f {}; using H::f; };
struct J4 : H {
  using H::f;
  void f(int);    // hides H::f(int)
  using H::g;
  void g(long);   // overloads H::g(int)
};
struct O { enum E { A, B }; };
struct J5 { int x; using enum O::E; };
static_assert(J5::A == 0);

void block() {
  enum e { a };
  using e::a;
}
enum G { g1 };
using ::g1;

namespace N { typedef int T; }
struct T {};
using N::T;
