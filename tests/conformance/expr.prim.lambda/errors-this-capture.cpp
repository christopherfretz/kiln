// [expr.prim.lambda.capture]/7, /10: `this` is captured only by a
// capture-default or an explicit capture; implicit capture by `[=]` is
// deprecated in C++20.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:26 error "'this' cannot be implicitly captured in this context"
//@ diag: 17:15 note "explicitly capture 'this'"
//@ diag: 18:28 warning "implicit capture of 'this' with a capture default of '=' is deprecated"
//@ diag: 18:16 note "add an explicit capture of 'this' to capture '*this' by reference"
//@ diag: 19:33 error "'this' cannot be implicitly captured in this context"
//@ diag: 19:22 note "explicitly capture 'this'"
//@ diag: 20:39 error "'this' cannot be implicitly captured in this context"
//@ diag: 20:28 note "explicitly capture 'this'"
//@ diag: 22:13 error "'this' cannot be captured in this context"
struct S {
  int m;
  void h() { [] { return m; }(); }
  void h2() { [=] { return m; }(); }
  void h3() { [&] { [] { return m; }(); }(); }
  void h4() { [=, this] { [] { return this; }(); }(); }
};
void f() { [this] {}(); }
