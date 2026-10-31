// [temp.variadic]/10 (fold expansion), [expr.const], spec/sema-templates.md
// §7.3 R-WALK and Q16: a binary left fold over a 65536-element pack yields a
// BinaryOperator spine 65536 deep. Substitution, the Oven (constant
// evaluation of `v`) and codegen (the run-time call in main) must handle it
// without exhausting the stack. clang 19 stops at -fbracket-depth (256), and
// with it raised it overflows its own stack, so this is kiln-only.
// The AST dump is not checked: with clang's indentation the text is
// quadratic in the depth (about 21 GB here).
//@ expect: run
//@ skip(clang): clang stops at its bracket depth, or overflows its stack
//@ oracle: none
//@ oven: 17 steps<=4 calls<=1 peak_bytes<=64
template <class T, T... Is> struct seq {};
template <int... Is> constexpr long long sum(seq<int, Is...>) {
  return (0LL + ... + Is);
}
using S = __make_integer_seq<seq, int, 65536>;
constexpr long long v = sum(S{});
static_assert(v == 65536LL * 65535 / 2);
int main() {
  long long r = sum(S{});
  return r == v ? 0 : 1;
}
