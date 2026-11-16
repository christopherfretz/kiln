// [lex.key]/1, [diff.cpp03.lex]: static_assert is an ordinary identifier in
// C++98, so a static_assert-declaration is not one there.
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ diag: 8:1 error "a type specifier is required for all declarations"

static_assert(1, "");
