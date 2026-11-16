// -fcontracts-client-check=all with -fcontracts-definition-check=off
// (GCC 16.1's flags; spec/contracts.md §6.1): the caller also checks
// the postconditions of a function it calls directly, after the call
// returns, with the result name bound to the call's result: a scalar,
// a reference and a class returned in memory. A function only declared
// in this translation unit is checked too; the caller needs nothing
// from the called function's definition. Found running GCC's C++
// testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-client-check=all -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: post 'r > x' 3
//@ stdout: post 'r.a[0] == 1' 0
//@ stdout: post '&r != &g' 1
//@ stdout: pre 'b' 1
//@ stdout: post 'r' 0
//@ stdout: done
#include <contracts>
#include <cstdio>
void handle_contract_violation(const std::contracts::contract_violation &v) {
  static const char *const kinds[] = {"?", "pre", "post", "assert"};
  std::printf("%s '%s' ", kinds[int(v.kind())], v.comment());
}
struct Big {
  int a[8];
};
int g;
int f(const int x) post(r: r > x) { return x; }
Big big(const int n) post(r: r.a[0] == 1) { return Big{{n}}; }
int &ref(const int i) post(r: &r != &g) { return i ? g : g; }
bool neg(const bool b) pre(b) post(r: r);
bool neg(const bool b) { return !b; }

int main() {
  std::printf("%d\n", f(3));
  std::printf("%d\n", big(0).a[0]);
  std::printf("%d\n", ref(1) = 1);
  std::printf("%d\n", neg(false));
  std::printf("%d\n", neg(true));
  std::puts("done");
}
