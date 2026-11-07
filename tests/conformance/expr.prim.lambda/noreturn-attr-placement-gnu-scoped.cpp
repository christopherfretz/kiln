// [expr.prim.lambda.general]/1, [dcl.fct]/1: `[[gnu::noreturn]]` after a
// lambda's parameter list or noexcept-specifier appertains to the call
// operator's type and is ignored there, as after a function declarator
// (clang -Wignored-attributes; clang 19 names it 'noreturn',
// noreturn-attr-placement-gnu-scoped-19.cpp). kiln applied it to the
// call operator.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ diag: 13:30 warning "attribute 'gnu::noreturn' ignored, because it cannot be applied to a type"
//@ diag: 14:39 warning "attribute 'gnu::noreturn' ignored, because it cannot be applied to a type"
void a13() { auto l = []() [[gnu::noreturn]] { __builtin_abort(); }; }
void a14() { auto l = []() noexcept [[gnu::noreturn]] { __builtin_abort(); }; }
