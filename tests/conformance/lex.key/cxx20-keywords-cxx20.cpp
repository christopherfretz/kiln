// [lex.key]/1: from C++20 on 'concept', 'consteval', 'constinit', 'co_await',
// 'co_return', 'co_yield', 'requires' and 'char8_t' are keywords, so none can
// name a variable (the C++17 counterpart is cxx20-keywords-cxx17.cpp; WP-1134).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 16:15 error "expected unqualified-id"
//@ diag: 17:15 error "expected unqualified-id"
//@ diag: 18:5 error "expected unqualified-id"
//@ diag: 19:5 error "expected unqualified-id"
//@ diag: 20:5 error "expected unqualified-id"
//@ diag: 21:5 error "expected unqualified-id"
//@ diag: 22:5 error "expected unqualified-id"
//@ diag: 23:5 error "cannot combine with previous 'int' declaration specifier"
//@ diag: 23:13 error "expected unqualified-id"
int consteval = 1;
int constinit = 2;
int concept = 3;
int requires = 4;
int co_await = 5;
int co_yield = 6;
int co_return = 7;
int char8_t = 8;
