// [dcl.array] variable length arrays (a GNU extension), [dcl.attr.noreturn]/2:
// a declaration of a variably modified type (a VLA, a pointer to a VLA) can
// follow a call to a `[[noreturn]]` function or a throw-expression, so it
// is never reached. kiln's codegen evaluated the array bound with no
// insertion point and crashed (WP-971); like clang (`EmitVariablyModifiedType`
// starts with `EnsureInsertPoint`) it now opens a fresh (unreachable) block
// first. The reachable twin `live` sizes both types at run time.
//@ expect: run
//@ strict-diags
//@ args: -std=gnu++26
//@ stdout: dead 4
//@ stdout: dead 5
//@ stdout: dead 6
//@ stdout: live 3 12 3
#include <cstdio>

[[noreturn]] __attribute__((noinline)) void fail(int v) { throw v; }

int vla_after_call(int n) {
  fail(n);
  int a[n];
  a[0] = 1;
  return a[0];
}

int vla_pointer_after_call(int n) {
  fail(n);
  int(*p)[n] = nullptr;
  return p ? 1 : 0;
}

int vla_after_throw(int n) {
  throw n;
  int a[n];
  a[0] = 1;
  return a[0];
}

void live(int n) {
  int a[n];
  auto p = &a;
  for (int i = 0; i < n; ++i)
    a[i] = i + 1;
  std::printf("live %d %d %d\n", int(sizeof a / sizeof a[0]), int(sizeof *p),
              (*p)[n - 1]);
}

int main() {
  try {
    vla_after_call(4);
  } catch (int e) {
    std::printf("dead %d\n", e);
  }
  try {
    vla_pointer_after_call(5);
  } catch (int e) {
    std::printf("dead %d\n", e);
  }
  try {
    vla_after_throw(6);
  } catch (int e) {
    std::printf("dead %d\n", e);
  }
  live(3);
}
