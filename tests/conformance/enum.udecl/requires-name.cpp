// [enum.udecl]/1: using enum names an enumeration
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:12 error "using enum requires an enum or typedef name"
using enum ;
