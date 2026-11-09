//@ expect: run
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
//@ stdout: 42 3 7 11 1
//@ stdout: 42 3 7 11 1
// [expr.const.init]/2, [class.temporary]/6 (C++26, P2686R5): the
// temporaries constexpr local references extend are automatic objects of
// each invocation at run time (on the stack, not static constants), and
// the references and pointers designate them.
extern "C" int printf(const char *, ...);
struct S { int a, b; };
void g() {
  constexpr const int &r = 42;
  constexpr const S &s = S{1, 2};
  constexpr const auto &[x, y] = S{3, 4};
  constexpr const int *p = &r;
  constexpr const int (&arr)[2] = {5, 6};
  const int *q = &r;
  printf("%d %d %d %d %d\n", *p, s.a + s.b, x + y, arr[0] + arr[1], q == p);
}
int main() {
  g();
  g();
}
