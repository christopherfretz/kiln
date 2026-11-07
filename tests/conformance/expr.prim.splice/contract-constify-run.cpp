// [expr.prim.splice]/2.5 (P3598R0), [expr.prim.id.unqual]/4 (P2900R14):
// in a contract predicate a splice designating a variable or a structured
// binding declared outside the assertion is const, so a member call
// through it picks the const overload: a parameter (pre), a
// namespace-scope variable (post), a reference, a local, a structured
// binding (contract_assert), and a data member through `this`. Outside
// a predicate the non-const overload. GCC 16.1 prints the same.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: const
//@ stdout: const
//@ stdout: non-const
//@ stdout: const
//@ stdout: const
//@ stdout: const
//@ stdout: const
//@ stdout: non-const
#include <contracts>
#include <cstdio>
#include <meta>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("violated: %s\n", v.comment());
}
struct W {
  int v = 1;
  int which() {
    std::puts("non-const");
    return v;
  }
  int which() const {
    std::puts("const");
    return v;
  }
};
struct H {
  W w;
  int m() pre(this->[:^^H::w:].which() > 0) { return 0; }
};
W gw;
int f(W w) pre([:^^w:].which() > 0) post([:^^gw:].which() > 0) {
  W& r = w;
  contract_assert([:^^r:].which() > 0);
  return w.which();
}
int main() {
  f(W{});
  W w;
  contract_assert([:^^w:].which() > 0);
  struct B {
    W a;
  };
  auto [x] = B{};
  contract_assert([:^^x:].which() > 0);
  H{}.m();
  [:^^w:].which();
}
