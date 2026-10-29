// [namespace.udecl]/1: a using-declaration ends with ';'
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:11 error "expected ';' after using declaration"
namespace N { int x; }
using N::x int y;
