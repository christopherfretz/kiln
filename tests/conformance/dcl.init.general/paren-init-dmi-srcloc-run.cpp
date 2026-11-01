// [dcl.init.general]/16.6.2.2, [class.mem.general]/11, [support.srcloc.cons]
// (WP-160): in a parenthesized aggregate initialization (P0960), a default
// member initializer's source-location builtins take the location of the
// closing `)` (or `}` for braces) of the initializer, as in clang 19: for
// a variable, a functional cast `S(x)` and `static_cast<S>(x)`, in constant
// and runtime evaluation alike. A C-style cast `(S)x` takes its operand's
// last token instead (clang `getEndLoc`; WP-182): a name, a call's `)`, a
// parenthesized operand's `)`, a member name, a postfix operator, the
// operand of a nested cast or a prefix operator.
//@ expect: run
//@ stdout: 36:18
//@ stdout: 39:8
//@ stdout: 41:8
//@ stdout: 42:8
//@ stdout: 45:7
//@ stdout: 48:8
//@ stdout: 50:7
//@ stdout: 52:7
//@ stdout: 54:7
//@ stdout: 55:10
//@ stdout: 56:9
//@ stdout: 57:9
//@ stdout: 58:18
//@ stdout: 59:13
struct S {
  int a;
  int line = __builtin_LINE();
  int col = __builtin_COLUMN();
};
extern "C" int printf(const char *, ...);
void p(const S &s) { printf("%d:%d\n", s.line, s.col); }
struct W { int m; };
int f(int x) { return x; }
int main() {
  constexpr S c(1);
  constexpr S d(1); static_assert(c.line == 35 && d.line == 36 && d.col == 18);
  p(d);
  p(S(
      4));
  p(static_cast<S>(
      5));
  S v(2);
  p(v);
  p(S
    (9));
  int x = 1;
  W w{3};
  p((S)x);
  p((S)
      x);
  p((S)f(
     2));
  p((S)(x + 1
      ));
  p((S)w.m);
  p((S)-x);
  p((S)x++);
  p((S)sizeof(int));
  p((S)(int)x);
  constexpr S k = (S)
     7;
  static_assert(k.line == 61 && k.col == 6);
}
