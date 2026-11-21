// [class.mfct]/1: clang's "out-of-line declaration of a member must be a
// definition" is a warning that is an error by default
// (-Wout-of-line-declaration); -Wno-error= keeps it a warning. WP-1835.
//@ args: -Wno-error=out-of-line-declaration
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:9 warning "out-of-line declaration of a member must be a definition"
//@ diag: 11:4 warning "out-of-line declaration of a member must be a definition"
struct D { void f(); D(); };
void D::f();
D::D();
