// [ACLE] Without sve in the features of the function a use is in, SVE
// types stay errors (clang checkTypeSupport): a lambda's call operator
// does not inherit the enclosing function's target attribute, so a
// parameter or local of SVE type, or a captured SVE variable, used in
// its body is an error, while its signature is checked when the lambda
// ends against the enclosing function's features. With 'sme' but no
// 'sve' the error is the non-streaming one. spec/sema.md "SVE".
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 24:38 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 25:31 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 25:35 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 26:26 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 26:35 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 30:75 error "SVE vector type '__SVInt8_t' cannot be used in a non-streaming function"
//@ diag: 30:43 error "SVE vector type '__SVInt8_t' cannot be used in a non-streaming function"
//@ diag: 30:43 error "SVE vector type '__SVInt8_t' cannot be used in a non-streaming function"
//@ diag: 32:72 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 32:43 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 32:43 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
__attribute__((target("sve"))) void lam(__SVInt8_t p) {
  __SVInt8_t v;
  auto l = [](__SVInt8_t x) { return x; };
  auto m = [&]() { __SVInt8_t y = v; (void)y; };
  auto n = [&]() { (void)v; (void)p; };
  (void)l; (void)m; (void)n;
}

__attribute__((target("sme"))) __SVInt8_t sme_only(__SVInt8_t a) { return a; }

__attribute__((target("crc"))) __SVInt8_t other(__SVInt8_t a) { return a; }
