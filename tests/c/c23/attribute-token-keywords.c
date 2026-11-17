// C23 6.7.13.2: an attribute-token's identifiers may be keywords; each of
// these is an unknown attribute, warned and ignored (clang 22). kiln's C
// and C++ parsers share the attribute-list code. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ std: c23
//@ requires: clang>=22
//@ strict-diags
//@ diag: 14:3 warning "unknown attribute 'const' ignored"
//@ diag: 15:3 warning "unknown attribute 'x::int' ignored"
//@ diag: 15:11 warning "unknown attribute 'static' ignored"
//@ diag: 16:3 warning "unknown attribute 'using' ignored"
//@ diag: 17:3 warning "unknown attribute 'x::y' ignored"
[[const]] int a1;
[[x::int, static]] int a2;
[[using]] int a3;
[[x::y(1, {2}, [3])]] int a4;
[[gnu::const]] int f(void);
