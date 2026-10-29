// [enum.udecl]/1: the enumeration needs a reachable enum-specifier
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "enumeration 'F' is incomplete"
//@ diag: 6:6 note "declared here"
enum F : int;
using enum F;
