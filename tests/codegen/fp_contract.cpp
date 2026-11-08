// [expr.pre]/6 lets a floating expression be evaluated with greater
// precision; clang's C++ default -ffp-contract=on (C23 7.12.2) fuses
// a * b + c within one expression into llvm.fmuladd, never across
// statements, and the STDC FP_CONTRACT / clang fp contract pragmas set
// the mode per compound statement or from the file-scope point on
// (spec/codegen.md "Floating-point semantics", WP-806).
float one(float a, float b, float c) { return a * b + c; }
double sub(double a, double b, double c) { return a * b - c + (c - a * b); }
float neg(float a, float b, float c) { return -(a * b) + c; }
float stmts(float a, float b, float c) { float t = a * b; return t + c; }
float assign(float a, float b, float c) { c += a * b; return c; }
double cast(float a, float b, double c) { return (double)(a * b) + c; }
float paren(float a, float b, float c, float d) { return (a * b + c) * d + a; }
float on(float a, float b, float c) {
#pragma STDC FP_CONTRACT ON
  return a * b + c;
}
float off(float a, float b, float c) {
#pragma STDC FP_CONTRACT OFF
  return a * b + c;
}
float dflt(float a, float b, float c) {
#pragma STDC FP_CONTRACT DEFAULT
  return a * b + c;
}
float nested(float a, float b, float c) {
#pragma clang fp contract(fast)
  float r = a * b + c;
  {
#pragma clang fp contract(off)
    r += a * b;
  }
  {
#pragma clang fp contract(on)
    r += a * b;
  }
  return r + a * b;
}
template <class T> T tmpl(T a, T b, T c) {
#pragma STDC FP_CONTRACT OFF
  return a * b + c;
}
template double tmpl(double, double, double);
#pragma clang fp contract(off)
float file_off(float a, float b, float c) { return a * b + c; }
#pragma clang fp contract(fast)
float file_fast(float a, float b, float c) { return a * b + c; }
