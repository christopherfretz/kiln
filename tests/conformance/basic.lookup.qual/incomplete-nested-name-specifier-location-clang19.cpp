// [expr.prim.id.qual]/1: an incomplete class in a multi-component
// nested-name-specifier; clang 19 reports at the incomplete component
// (the clang 22 shape is incomplete-nested-name-specifier-location.cpp).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:4 error "incomplete type 'N::F2' named in nested name specifier" dup(clang)
//@ diag: 15:22 note "forward declaration of 'N::F2'"
//@ diag: 17:12 error "incomplete type 'N::F2' named in nested name specifier"
//@ diag: 15:22 note "forward declaration of 'N::F2'"
//@ diag: 18:9 error "incomplete type 'N::F2' named in nested name specifier"
//@ diag: 15:22 note "forward declaration of 'N::F2'"
//@ diag: 20:4 error "implicit instantiation of undefined template 'H::M<int>'" dup(clang)
//@ diag: 19:35 note "template is declared here"
namespace N { struct F2; }
N::F2::x nn;
int i = N::F2::G::y;
void N::F2::g() {}
struct H { template<class> struct M; };
H::M<int>::x q;
