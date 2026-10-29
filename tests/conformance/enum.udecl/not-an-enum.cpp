// [enum.udecl]/1: the using-enum-declarator names an enumeration
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:15 error "'N::S' is not an enumerated type"
namespace N { struct S {}; }
using enum N::S;
