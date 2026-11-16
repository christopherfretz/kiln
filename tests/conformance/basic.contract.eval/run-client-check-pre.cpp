// -fcontracts-client-check=pre with -fcontracts-definition-check=off
// (GCC 16.1's flags; spec/contracts.md §6.1): the caller checks the
// preconditions of each function it calls directly, a free function, a
// non-static member function (its predicate's `this` is the object
// argument) and a static member function, at the call, after the
// arguments; the violation names the called function and the
// precondition's own line. The called function checks nothing, so a
// postcondition, a constructor's precondition and a call through a
// pointer to function go unchecked; a contract_assert in a body is
// always checked. Found running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-client-check=pre -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: pre 'x > 0' in int f(int) at 6
//@ stdout: body f 0
//@ stdout: pre 'v > n' in int S::get(int) const at 14
//@ stdout: pre 'n < 0' in static int S::make(int) at 15
//@ stdout: body f -1
//@ stdout: assert 'x != -1' in int f(int) at 8
//@ stdout: done
#include <contracts>
#include <cstdio>
const unsigned base = __LINE__;
void handle_contract_violation(const std::contracts::contract_violation &v) {
  static const char *const kinds[] = {"?", "pre", "post", "assert"};
  std::printf("%s '%s' in %s at %u\n", kinds[int(v.kind())], v.comment(),
              v.location().function_name(), v.location().line() - base);
}
int f(int x) pre(x > 0) post(r: r > 5) {
  std::printf("body f %d\n", x);
  contract_assert(x != -1);
  return x;
}
struct S {
  int v;
  S(int a) pre(a > 0) : v(a) {}
  int get(int n) const pre(v > n) { return v; }
  static int make(int n) pre(n < 0);
};
int S::make(int n) { return n; }

int main() {
  f(0);
  S s(-3);
  s.get(1);
  S::make(2);
  int (*p)(int) = f;
  p(-1);
  std::puts("done");
}
