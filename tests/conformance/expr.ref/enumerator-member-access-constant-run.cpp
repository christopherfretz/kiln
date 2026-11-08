// [expr.ref]/7.5, [expr.const]: `E1.E2` naming an enumerator is a constant
// expression when `E1` is: the object expression is evaluated as a
// glvalue, nothing is read, and the result is discarded (clang
// `VisitIgnoredBaseExpression`). So `case response.INITIAL:` with
// `response` a non-constexpr local or parameter is valid. WP-811 (gRPC's
// grpclb.cc).
//@ expect: run
//@ strict-diags
//@ stdout: 2 5 6 14
extern "C" int printf(const char *, ...);
struct R {
  enum T { A, B = 5, C };
  T type;
  static constexpr int K = 7;
};
R g{R::C};
constexpr R *gp = &g;
int f(R response) {
  int n = 0;
  switch (response.type) {
  case response.A: n += 1; break;
  case response.B: n += 2; break;
  case gp->C: n += 3; break;
  }
  static_assert(response.B == 5 && response.K == 7 && gp->C == 6);
  return n;
}
int main() {
  R local{R::A};
  constexpr int b = local.B;
  int arr[local.C];
  printf("%d %d %d %d\n", f(R{R::B}), b, (int)sizeof arr / (int)sizeof(int),
         f(local) + f(g) + local.B + gp->B + R::A);
}
