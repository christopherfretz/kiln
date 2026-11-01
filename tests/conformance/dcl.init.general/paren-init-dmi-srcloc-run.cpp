// [dcl.init.general]/16.6.2.2, [class.mem.general]/11, [support.srcloc.cons]
// (WP-160): in a parenthesized aggregate initialization (P0960), a default
// member initializer's source-location builtins take the location of the
// closing `)` (or `}` for braces) of the initializer, as in clang 19: for
// a variable, a functional cast `S(x)` and `static_cast<S>(x)`, in constant
// and runtime evaluation alike. (A C-style cast `(S)x` uses its operand's
// end in clang; kiln does not record it yet.)
//@ expect: run
//@ stdout: 23:18
//@ stdout: 26:8
//@ stdout: 28:8
//@ stdout: 29:8
//@ stdout: 32:7
struct S {
  int a;
  int line = __builtin_LINE();
  int col = __builtin_COLUMN();
};
extern "C" int printf(const char *, ...);
void p(const S &s) { printf("%d:%d\n", s.line, s.col); }
int main() {
  constexpr S c(1);
  constexpr S d(1); static_assert(c.line == 22 && d.line == 23 && d.col == 18);
  p(d);
  p(S(
      4));
  p(static_cast<S>(
      5));
  S v(2);
  p(v);
  p(S
    (9));
}
