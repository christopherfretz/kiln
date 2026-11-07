// [dcl.decl.general]/4, [dcl.attr.noreturn] (GNU form): calls through
// pointers whose GNU `noreturn` is written after a grouping `(` or in a
// type-id reach codegen, and control does not return from them (no
// -Wreturn-type for the non-void function that ends in one) (WP-708).
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ stdout: pick 2
//@ stdout: fail 7
#include <cstdio>
#include <cstdlib>
__attribute__((noreturn)) void fail(int code) {
  std::printf("fail %d\n", code);
  std::exit(0);
}
void (__attribute__((noreturn)) *on_fail)(int) = fail;
using Handler = void (*__attribute__((noreturn)))(int);
int pick(int x, Handler h) {
  if (x > 0) {
    std::printf("pick %d\n", x);
    return x;
  }
  h(7);
}
int main() {
  int r = pick(2, on_fail);
  pick(-r, on_fail);
  return 1;
}
