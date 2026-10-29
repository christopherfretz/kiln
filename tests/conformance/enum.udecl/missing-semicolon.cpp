// [enum.udecl]/1: a using-enum-declaration ends with ';'
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:13 error "expected ';' after using-enum declaration"
enum E { A };
using enum E int z;
