// [expr.prim.lambda.capture]/8 (the standard's own example) with
// [basic.def.odr]/5: in `*&n` no lvalue-to-rvalue conversion is applied to
// `n`, so `&n` odr-uses it and `[=]` captures it; the name then denotes the
// closure member ([expr.prim.id.unqual]/3, [expr.prim.lambda.capture]/12),
// and reading it needs the closure object, which a constant expression
// evaluated in the body does not have ([expr.const]/10.1). Found running
// GCC's C++ testsuite; clang 19 and 22 accept: the text and GCC 16 decide.
//@ xfail(clang): clang 19 and 22 evaluate a captured constant's own value
//@ expect: compile-fail
//@ diag: 15:19 error "constexpr variable 'j' must be initialized by a constant expression"
void g() {
  const int n = 0;
  [=] {
    constexpr int i = n;
    constexpr int j = *&n;
    return i + j;
  };
}
