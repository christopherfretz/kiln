// [dcl.init.general]/16.6.2.2, [support.srcloc.cons] (WP-203): in a C-style
// cast paren aggregate initialization `(S)a[i]`, a default member
// initializer's source-location builtins take the subscript's `]` (clang
// `ArraySubscriptExpr::getEndLoc`), also in an instantiation and in
// constant evaluation.
//@ expect: run
//@ stdout: 25:11
//@ stdout: 27:9
//@ stdout: 29:9
//@ stdout: 31:6
//@ stdout: 21:7
struct S {
  int a;
  int line = __builtin_LINE();
  int col = __builtin_COLUMN();
};
extern "C" int printf(const char *, ...);
void p(const S &s) { printf("%d:%d\n", s.line, s.col); }
template <class T> void t(T *a, int x) {
  p((S)a[
     x]);
}
int main() {
  int a[3] = {1, 2, 3}, x = 1;
  p((S)a[x]);
  p((S)a[
       x]);
  p((S)a[x][
       a]);
  p((S)x[a
     ]);
  t(a, 1);
  constexpr int ca[2] = {4, 5};
  constexpr S k = (S)ca[
      1];
  static_assert(k.line == 35 && k.col == 8);
}
