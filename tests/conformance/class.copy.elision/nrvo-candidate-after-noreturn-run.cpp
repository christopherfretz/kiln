// [class.copy.elision]/1, [dcl.attr.noreturn]/2, [expr.throw]: the
// declaration of a named local that is a return-value-elision candidate can
// follow a call to a `[[noreturn]]` function or a throw-expression, so it
// is never reached. kiln's codegen stored the variable's elision flag with
// no insertion point and crashed (WP-971); like clang it now opens a fresh
// (unreachable) block first. The reachable twin `live` still elides the
// copy.
//@ expect: run
//@ strict-diags
//@ stdout: dead 1
//@ stdout: dead 2
//@ stdout: live 3 copies 0 alive 0
#include <cstdio>

static int copies = 0, alive = 0;

struct S {
  int v;
  S(int x) : v(x) { ++alive; }
  S(const S &o) : v(o.v) {
    ++copies;
    ++alive;
  }
  ~S() { --alive; }
};

[[noreturn]] __attribute__((noinline)) void fail(int v) { throw v; }

S after_call(int v) {
  fail(v);
  S x(v);
  return x;
}

S after_throw(int v) {
  throw v;
  S x(v);
  return x;
}

S live(int v) {
  S x(v);
  return x;
}

int main() {
  try {
    after_call(1);
  } catch (int e) {
    std::printf("dead %d\n", e);
  }
  try {
    after_throw(2);
  } catch (int e) {
    std::printf("dead %d\n", e);
  }
  {
    S s = live(3);
    std::printf("live %d copies %d", s.v, copies);
  }
  std::printf(" alive %d\n", alive);
}
