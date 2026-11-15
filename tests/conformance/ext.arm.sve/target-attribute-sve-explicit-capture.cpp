// [ACLE] SVE types in a function without the `sve` feature: an explicit
// capture names the variable in the lambda's call operator, which does
// not inherit the enclosing function's `target("sve")`, so clang's
// "SVE vector type ... cannot be used in a target without sve" is given
// at the capture (by reference and by copy, then the sizeless by-copy
// error), as at each use in the body (clang DiagnoseUseOfDecl in the
// call operator's context). Found in downstream code.
// spec/types.md "Per-function features".
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 20:14 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 21:14 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 21:25 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 22:24 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 23:13 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 23:13 error "by-copy capture of variable 'v' with sizeless type '__SVInt32_t'"
__attribute__((target("sve"))) void f() {
  __SVInt32_t v;
  auto a = [&v] { };
  auto b = [&v] { (void)v; };
  auto c = [&] { (void)v; };
  auto d = [v] { };
  (void)a; (void)b; (void)c; (void)d;
}
