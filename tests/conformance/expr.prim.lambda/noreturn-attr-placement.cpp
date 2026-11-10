// [expr.prim.lambda.general]/1, [dcl.fct]/1: the placements of GNU
// `noreturn` and `[[noreturn]]` a lambda-declarator accepts (clang
// `ParseLambdaSpecifiers`): a GNU attribute right after the parameter
// list (or the introducer, without one), ahead of the lambda-specifiers and
// the noexcept-specifier; `[[noreturn]]` ahead of the parameter list.
//@ expect: compile-ok
//@ skip(gcc): clang's GNU-attribute placements in a lambda-declarator, which GCC 16.1 rejects
//@ strict-diags
//@ std: c++23
void a2() { auto l = []() __attribute__((noreturn)) noexcept { __builtin_abort(); }; }
void a3() { auto l = []() __attribute__((noreturn)) -> void { __builtin_abort(); }; }
void a8() { auto l = [] [[noreturn]] () { __builtin_abort(); }; }
void a15() { auto l = [] __attribute__((noreturn)) { __builtin_abort(); }; }
void a16() { auto l = []() __attribute__((noreturn)) mutable noexcept { __builtin_abort(); }; }
void a17() { auto l = [] [[noreturn]] () mutable noexcept -> void { __builtin_abort(); }; }
