// [namespace.udecl]/13, [over.match.list]/1.1, [dcl.init.list]/3.7: an
// inherited initializer-list constructor is a candidate of the first
// phase of list-initialization, so `D{1, 2}` prefers it to the inherited
// B(int, int) (which parentheses still select).
//@ expect: compile-ok
#include <initializer_list>
struct B {
  constexpr B(std::initializer_list<int> l) : v(int(l.size())) {}
  constexpr B(int, int) : v(-1) {}
  int v;
};
struct D : B { using B::B; };
constexpr D d{1, 2, 3};
static_assert(d.v == 3);
constexpr D e{1, 2};
static_assert(e.v == 2);
constexpr D g(1, 2);
static_assert(g.v == -1);
