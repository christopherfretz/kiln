// [basic.contract.eval]/7-10 (P2900R14): under observe a violated
// precondition, postcondition or contract_assert (detection mode
// predicate_false) invokes the contract-violation handler, here a
// user-replaced one ([basic.contract.handler]/3), and execution continues
// after it. The contract_violation describes the assertion: its kind,
// semantic, mode, the predicate's text and the assertion's line
// (printed relative to `base`). A precondition runs before the body, a
// postcondition after the result is initialised with the result name
// bound to it. This is run-observe.cpp under libc++, which ships no
// <contracts>, so kiln's own header and its contract_violation layout
// serve (spec/contracts.md §6.3).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -stdlib=libc++
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: pre 2 1 'x > 0' 6
//@ stdout: body -1
//@ stdout: post 2 1 'r > x' 6
//@ stdout: assert 2 1 'y != 3' 10
//@ stdout: pre 2 1 'v.size() < 3' 16
//@ stdout: done 0 3
#include <contracts>
#include <cstdio>
const unsigned base = __LINE__;
void handle_contract_violation(const std::contracts::contract_violation& v) {
  static const char* const kinds[] = {"?", "pre", "post", "assert"};
  std::printf("%s %d %d '%s' %u\n", kinds[int(v.kind())], int(v.semantic()), int(v.mode()),
              v.comment(), v.location().line() - base);
}
int f(const int x) pre(x > 0) post(r: r > x) {
  std::printf("body %d\n", x);
  return x;
}
int g(int y) { contract_assert(y != 3); return y; }
struct V {
  int n;
  int size() const { return n; }
};
struct C {
  int m(const V& v) pre(v.size() < 3) { return v.size(); }
};
int main() {
  int a = f(-1) + 1;
  int b = g(3);
  C c;
  int d = c.m(V{3});
  std::printf("done %d %d\n", a, d);
  return b - 3;
}
