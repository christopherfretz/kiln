// [expr.prim.splice]/2.3 and /2.5 (P3598R0): only a splice designating a
// variable or structured binding is const-ified in a contract predicate;
// one designating an object keeps the object's type. P3598R0 names this
// the visible way around const-ification: "++[: std::meta::object_of(^^p)
// :]" (with "code smell!"). So the predicate below may modify the
// variable through `object_of`, and its non-const overload is chosen.
// GCC 16.1 const-ifies the object splice too ("increment of read-only
// location"), against the wording (spec/contracts.md §9), so there is no
// oracle.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: non-const
//@ stdout: 2
#include <contracts>
#include <cstdio>
#include <meta>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("violated: %s\n", v.comment());
}
struct W {
  int which() {
    std::puts("non-const");
    return 1;
  }
  int which() const {
    std::puts("const");
    return 1;
  }
};
int g = 1;
W gw;
void f() pre(++[:std::meta::object_of(^^g):] > 0) pre([:std::meta::object_of(^^gw):].which() > 0) {}
int main() {
  f();
  std::printf("%d\n", g);
}
