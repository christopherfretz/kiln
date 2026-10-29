// [namespace.udecl]/8: a using-declaration outside a class names no class
// member, except an enumerator (the note suggests an alias for a type)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:10 error "using declaration cannot refer to class member"
//@ diag: 11:10 error "using declaration cannot refer to class member"
//@ diag: 11:7 note "use an alias declaration instead"
//@ diag: 12:10 error "using declaration cannot refer to class member"
struct S { int m; typedef int T; enum E { A }; };
using S::m;
using S::T;
using S::nope;
using S::A;
int k = A;
