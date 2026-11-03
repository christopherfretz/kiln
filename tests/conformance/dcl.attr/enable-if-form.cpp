// clang's `enable_if` attribute (clang `checkFunctionConditionAttr`): two
// arguments, a condition contextually converted to `bool` and a string
// literal message.
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:43 error "expected string literal as argument of 'enable_if' attribute"
//@ diag: 11:30 error "'enable_if' attribute requires exactly 2 arguments"
//@ diag: 13:38 error "value of type 'P' is not contextually convertible to 'bool'"
void f(int n) __attribute__((enable_if(n, 1)));
void g(int n) __attribute__((enable_if(n)));
struct P {};
void h(P p) __attribute__((enable_if(p, "")));
