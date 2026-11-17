// [expr.prim.lambda.capture]/8 (the standard's own example) with
// [basic.def.odr]/5: in `*&n` no lvalue-to-rvalue conversion is applied to
// `n`, so `&n` odr-uses it and `[=]` captures it; the name then denotes the
// closure member ([expr.prim.id.unqual]/3, [expr.prim.lambda.capture]/12),
// and reading it needs the closure object, which a constant expression
// evaluated in the body does not have ([expr.const]/10.1). Found running
// GCC's C++ testsuite; clang 19 and 22 accept: the text and GCC 16 decide.
//@ xfail(clang): clang 19 and 22 evaluate a captured constant's own value
//@ xfail(kiln): WP-1532 remainder: the Oven reads a by-copy capture named in a constant expression in the lambda body as the enclosing variable (kiln_oven/src/compile.rs captured_var_place fallback; WP-1526's area)
//@ expect: compile-fail
//@ diag: 16:19 error "constexpr variable 'j' must be initialized by a constant expression"
void g() {
  const int n = 0;
  [=] {
    constexpr int i = n;
    constexpr int j = *&n;
    return i + j;
  };
}
