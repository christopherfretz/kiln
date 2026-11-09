//@ expect: run
//@ args: -freflection
//@ oracle: none
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 1 1 4
// [basic.types.general] P2996R13: an object of consteval-only type exists
// only during constant evaluation, so a namespace-scope consteval-only
// variable with external linkage (`extern constexpr`, a consteval-only
// class, an array of reflections) has no storage and is not emitted; its
// uses are all constant (spec/codegen.md "Consteval-only variables",
// WP-1047).
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
struct S {};
struct H {
  info i;
  int n;
};
extern constexpr info r = ^^S;
extern constexpr H h{^^int, 4};
extern constexpr info rs[2] = {^^S, ^^int};
inline constexpr info ri = ^^S;
int main() {
  constexpr bool a = r == ^^S, b = rs[1] == h.i && ri == r;
  constexpr int n = h.n;
  printf("%d %d %d\n", a, b, n);
}
