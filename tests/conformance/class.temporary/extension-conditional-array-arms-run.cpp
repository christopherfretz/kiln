// [class.temporary]/6, [expr.cond]/4-5: a reference bound to a glvalue
// conditional expression whose second and third operands are array
// xvalues of different temporaries (`c ? X{..}.a : X{..}.a`) extends the
// temporary of the arm that was evaluated, and only that one exists: it
// is destroyed exactly once, in reverse order of its elements, at the end
// of the reference's lifetime, for either branch; when a later
// declaration's initializer throws, the stack unwinding destroys it
// ([except.ctor]/2). Also a class prvalue conditional, which is
// materialized once. Found running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
//@ stdout: +1 +2 u1 -2 -1 | +3 +4 u3 -4 -3 | +5 +6 -6 -5 caught | +7 +8 -8 -7 caught | +9 u9 -9 | +11 +12 u12 -12 -11 | live 0
extern "C" int printf(const char*, ...);
int live;
struct S {
  int v;
  S(int v) : v(v) { ++live; printf("+%d ", v); }
  S(const S&) = delete;
  ~S() { --live; printf("-%d ", v); }
};
struct X {
  S a[2];
};
struct T {
  T() { throw 1; }
};
void f(bool c) {
  const S (&r)[2] = c ? X{{1, 2}}.a : X{{3, 4}}.a;
  printf("u%d ", r[0].v);
}
void g(bool c) {
  S (&&r)[2] = c ? static_cast<S (&&)[2]>(X{{5, 6}}.a) : static_cast<S (&&)[2]>(X{{7, 8}}.a);
  T t;
  printf("unreached %d ", r[0].v);
}
void h(bool c) {
  const S& r = c ? S(9) : S(10 + int(c));
  printf("u%d ", r.v);
}
void k(bool c) {
  const S& r = (c ? X{{11, 12}} : X{{13, 14}}).a[1];
  printf("u%d ", r.v);
}
int main() {
  f(true);
  printf("| ");
  f(false);
  printf("| ");
  try { g(true); } catch (int) { printf("caught "); }
  printf("| ");
  try { g(false); } catch (int) { printf("caught "); }
  printf("| ");
  h(true);
  printf("| ");
  k(true);
  printf("| live %d\n", live);
}
