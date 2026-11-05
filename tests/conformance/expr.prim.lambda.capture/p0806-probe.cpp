// [expr.prim.lambda.capture]/7, [depr.capture.this] (C++20, P0806R2 and
// P0409R2): `[=, this]` is allowed, implicit capture of `this` by `[=]`
// is deprecated. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 14:27 warning "implicit capture of 'this' with a capture default of '=' is deprecated"
//@ diag: 14:15 note "add an explicit capture of 'this' to capture '*this' by reference"
struct S {
  int v = 3;
  int f() {
    auto a = [=, this] { return v; };
    auto b = [=, *this] { return v; };
    auto c = [=] { return v; };
    return a() + b() + c();
  }
};
int use() { return S{}.f(); }
