// [dcl.init.list]/3.5: an empty list value-initializes a class with a
// default constructor even when it has an initializer-list constructor;
// without a default constructor the initializer-list constructor gets
// an empty list.
#include "Inputs/initializer_list.h"
struct A {
  int how;
  constexpr A() : how(0) {}
  constexpr A(std::initializer_list<int>) : how(1) {}
};
static_assert(A{}.how == 0);
static_assert(A{{}}.how == 1);
struct B {
  int n;
  constexpr B(std::initializer_list<int> l) : n(int(l.size())) {}
};
static_assert(B{}.n == 0);
constexpr B b = {};
static_assert(b.n == 0);
