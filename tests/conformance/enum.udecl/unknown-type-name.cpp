// [enum.udecl]/1: the using-enum-declarator is a type-name (not a variable
// or a namespace)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:12 error "unknown type name Undeclared"
//@ diag: 10:15 error "unknown type name v"
//@ diag: 11:15 error "unknown type name M"
namespace N { int v; namespace M {} }
using enum Undeclared;
using enum N::v;
using enum N::M;
