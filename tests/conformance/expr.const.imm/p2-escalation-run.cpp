// [expr.const.imm]/2.1, /4.2 (P4101R1): an immediate constant expression
// in an immediate-escalating function makes it an immediate function,
// even when the expression is itself a constant expression (`R ==
// ^^int`, `^^T == ^^int`); a call of the escalated function is then an
// immediate invocation and its (non-consteval-only) result reaches run
// time as a constant. The same for a member of a template parameter
// object holding a reflection and for a lambda with an `info` parameter.
// Compared with GCC 16.1.
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ stdout: 7 5 2 1
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
template <info R> constexpr int f() { return R == ^^int ? 7 : 1; }
struct S { info r; int n; };
template <S s> constexpr int g() { return s.n + (s.r == ^^int); }
template <class T> constexpr int h(T x) { return x + (^^T == ^^int); }

int main() {
  auto lam = [](info r) { return r == ^^int; };
  int a = f<^^int>();
  int b = g<S{^^int, 4}>();
  int c = h(1);
  bool d = lam(^^int);
  printf("%d %d %d %d\n", a, b, c, d);
  return 0;
}
