// [namespace.udecl]/1: the named member must exist (each declarator of a list)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:10 error "no member named 'nope' in namespace 'N'"
//@ diag: 8:16 error "no member named 'nope2' in namespace 'N'"
namespace N { int x; }
using N::nope;
using N::x, N::nope2;
int k = x;
