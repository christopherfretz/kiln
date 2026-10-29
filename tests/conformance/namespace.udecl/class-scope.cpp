// [namespace.udecl]/3: in a class, the nested-name-specifier names a base
// class (kiln has no base classes yet: every non-enum qualifier is diagnosed)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:25 error "using declaration in class refers into 'N::', which is not a class"
//@ diag: 12:37 error "using declaration refers to its own class"
//@ diag: 12:49 error "using declaration in class refers into '::N::', which is not a class"
//@ diag: 13:18 error "using declaration refers into 'D::', which is not a base class of 'E'"
//@ diag: 14:18 error "using declaration refers to its own class"
//@ diag: 14:21 error "no member named 'q' in 'F'"
namespace N { int x; }
struct D { int a; using N::x; using D::a; using ::N::x; };
struct E { using D::a; };
struct F { using F::q; };
