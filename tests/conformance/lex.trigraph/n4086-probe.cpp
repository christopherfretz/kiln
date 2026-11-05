// [lex.phases]/1, [diff.cpp14.lex] (C++17, N4086): trigraphs are removed;
// `??=` is three ordinary characters. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++17
//@ args: -Wno-trigraphs
//@ strict-diags
static_assert(sizeof("??=") == 4, "no trigraph replacement");
static_assert("??("[2] == '(', "trigraph text is kept");
