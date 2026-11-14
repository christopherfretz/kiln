// [expr.const.const]/1-2 (P4101R1): an object whose reflections are all
// null is not an immediate object, so its other members may be read at
// run time; a `constexpr` one is an ordinary constant object, its
// `std::meta::info` members null at run time (spec/codegen.md "Constexpr
// locals", "Run-time reflections"), and each scalar member read folds.
// kiln only: GCC 16.1 rejects the run-time reads under the
// P2996R13 type rule (docs/status/deviations.md).
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ stdout: global local 3
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
struct S { info r; const char* name; int n; };
struct Outer { S inner; int k; };

constexpr S gs{{}, "global", 1};
constexpr Outer go{{{}, "unused", 2}, 0};

int main() {
  constexpr S s{{}, "local", 2};
  (void)s;
  printf("%s %s %d\n", gs.name, s.name, gs.n + go.inner.n + go.k);
  return 0;
}
