// [lex.key]/1, [diff.cpp03.lex]: 'nullptr' is a keyword only from C++11 on;
// in C++98 it is an ordinary identifier, so a use with no declaration is 'use of
// undeclared identifier' and a variable may be named 'nullptr' (WP-421).
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ diag: 9:10 error "use of undeclared identifier 'nullptr'"
//@ diag: 10:21 error "use of undeclared identifier 'nullptr'"
int *p = nullptr;
void f() { if (p == nullptr) {} }
namespace N { int nullptr = 0; }
bool b = N::nullptr == 0;
