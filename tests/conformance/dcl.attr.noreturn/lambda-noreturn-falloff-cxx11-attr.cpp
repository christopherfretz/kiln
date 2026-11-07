// [dcl.attr.noreturn]/2: as lambda-noreturn-falloff.cpp, for a lambda
// declared `[[noreturn]]` ahead of its parameter list (C++23).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:74 error "lambda declared 'noreturn' should not return"
void f(int x) { auto l = [x] [[noreturn]] () { if (x) __builtin_abort(); }; }
