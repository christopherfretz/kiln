// [expr.prim.this]/2-3, [dcl.contract.func]: a member function of a local
// class declared in a lambda in a contract predicate has its own `this`,
// neither the enclosing class's nor const: [expr.prim.this]/2's const
// `this` is the predicate's, and [expr.prim.id.general]/2's rule is about
// the constructor's class. Found running GCC's C++ testsuite
// (g++.dg/contracts).
//@ expect: run
//@ args: -fcontracts -fcontract-evaluation-semantic=enforce
//@ stdout: ok
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
#include <cstdio>
struct S {
  int k = 0;
  S(int) pre([] {
    struct In {
      bool i = true;
      bool f() const pre(i) { return i; }
      bool flip() { i = !i; return !i; }
      In() post(this->i) {}
    };
    In in;
    return in.f() && in.flip();
  }()) {}
  ~S() post([] {
    struct In { int n = 1; int g() { return ++n; } } in;
    return in.g() == 2;
  }()) {}
};
int main() {
  S s(1);
  std::puts("ok");
}
