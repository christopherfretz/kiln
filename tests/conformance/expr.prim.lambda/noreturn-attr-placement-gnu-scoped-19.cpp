// [expr.prim.lambda.general]/1: noreturn-attr-placement-gnu-scoped.cpp
// at clang 19, which names the attribute as written, without its scope.
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ diag: 9:30 warning "attribute 'noreturn' ignored, because it cannot be applied to a type"
//@ diag: 10:39 warning "attribute 'noreturn' ignored, because it cannot be applied to a type"
void a13() { auto l = []() [[gnu::noreturn]] { __builtin_abort(); }; }
void a14() { auto l = []() noexcept [[gnu::noreturn]] { __builtin_abort(); }; }
