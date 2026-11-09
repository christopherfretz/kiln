// [class.dtor] (C++11): a destructor cannot be declared `constexpr`
// (clang `err_constexpr_dtor`). clang keeps the specifier, so a C++11
// definition also gets the `void` return type error of [dcl.constexpr]/3
// ([basic.types]/10: `void` is not a literal type before C++14; WP-1123).
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 12:12 error "destructor cannot be declared constexpr"
//@ diag: 12:22 error "constexpr function's return type 'void' is not a literal type"
//@ diag: 13:12 error "destructor cannot be declared constexpr"

struct A { constexpr ~A() {} };
struct B { constexpr ~B(); };
