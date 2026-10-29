// [namespace.alias]/1: the qualified-namespace-specifier names a namespace
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "expected namespace name"
//@ diag: 10:18 error "expected namespace name"
//@ diag: 11:15 error "expected namespace name"
namespace N { int x; }
int y;
namespace A = y;
namespace B = N::x;
namespace C = nope;
