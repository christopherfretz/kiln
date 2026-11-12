// [expr.const.const]/1-2 (P4101R1): an object whose reflections are all
// null is not an immediate object, so its other members may be read at
// run time; a `constexpr` one still gets no storage while codegen has no
// run-time `std::meta::info` (spec/codegen.md "Consteval-only
// variables"): each scalar member read folds, and a discarded name of it
// emits nothing. kiln only: GCC 16.1 rejects the run-time reads under the
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
