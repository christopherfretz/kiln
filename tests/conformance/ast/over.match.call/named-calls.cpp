//@ expect: ast
//@ oracle: clang
// [over.match.call]: calls through an overload set name the selected
// function with an ImplicitCastExpr to a pointer around its DeclRefExpr;
// a qualified call keeps the qualifier; the chosen candidate's argument
// conversions are applied. (Calls resolved through ADL are not here: clang
// marks them `adl`, which kiln's dump has no field for yet.)
namespace N {
struct A {};
int f(A, int) { return 2; }
int f(A, double) { return 3; }
} // namespace N
int g(int) { return 3; }
int g(double) { return 4; }
int g(const char *) { return 5; }
int t(N::A a, short s) {
  return g(s) + g(1.5f) + g("x") + N::f(a, 'c') + N::f(a, 2.5f);
}
