// [expr.const]/4 (usable in constant expressions), [basic.def.odr]/5:
// a reference variable whose referent is a const-qualified constant is
// read as its value (clang `checkVarTypeForConstantEmission` returns
// `CEK_AsValueOrReference` for a reference to a const-emittable type, so
// `tryEmitAsConstant` folds the lvalue-to-rvalue conversion, WP-233).
// A local reference binds a temporary of automatic storage duration and
// is not folded; a reference to a non-const temporary is not either.
const int& a = {};
const int b = 5;
const int& c = 7;
constexpr const int& d = {9};
const double& f = 1.5;
int sum() { return a + b + c + d; }
int local() {
  const int& la = {}, lb = 5;
  return la + lb;
}
double fp() { return f * 2; }
