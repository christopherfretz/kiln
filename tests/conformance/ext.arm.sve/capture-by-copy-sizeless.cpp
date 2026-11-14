// [expr.prim.lambda.capture]/10, [ACLE]: an entity captured by copy
// becomes a member of the closure type, which a sizeless SVE type cannot
// be: "by-copy capture of variable 'v' with sizeless type", for an
// explicit and an implicit capture (clang captureInLambda,
// RequireCompleteSizedType); a capture by reference is fine.
// spec/types.md "Per-function features".
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
//@ strict-diags
//@ diag: 15:13 error "by-copy capture of variable 'v' with sizeless type '__SVInt8_t'"
//@ diag: 16:26 error "by-copy capture of variable 'v' with sizeless type '__SVInt8_t'"
__SVInt8_t g(__SVInt8_t);
void f(__SVInt8_t p) {
  __SVInt8_t v = p;
  auto a = [v]() {};
  auto b = [=]() { (void)v; };
  auto c = [&]() { return g(v); };
  (void)a; (void)b; (void)c;
}
