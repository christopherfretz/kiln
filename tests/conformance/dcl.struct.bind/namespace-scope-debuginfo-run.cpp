// [dcl.struct.bind]/1 at `-g` (WP-910): the variable a namespace-scope
// structured binding introduces has no name, so its DIGlobalVariable has
// only the `_ZDC...E` linkage name, as in clang (LLVM rejects an empty
// `name:`, which failed the compile). Static and reference forms, and a
// dynamically initialized one (spec/landed/codegen/WP-910.md).
//@ expect: run
//@ args: -g
//@ stdout: 21
extern "C" int printf(const char *, ...);
struct P { int x; long y; };
P p{1, 2};
auto [a, b] = p;
int arr[2] = {3, 4};
auto &[c, d] = arr;
static auto [e, f] = P{5, 6};
int main() {
  printf("%ld\n", a + b + c + d + e + f);
}
