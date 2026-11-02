// [dcl.fct]/22, [expr.prim.lambda.closure]/3: a lambda's own parameters
// may be `auto` (a generic lambda), a nested prototype's may not; nor may
// those of a nested prototype in a function template's clause, where a
// constrained `C auto` is no different. A nested parameter whose trailing
// return type is `auto` is reported at its decl-specifier.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:25 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 16:25 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 17:49 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 18:42 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 19:19 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 20:22 error "'auto' not allowed in function prototype that is not a function declaration"
template <class T> concept C = true;
auto l1 = [](void (*cb)(auto)) {};
auto l2 = [](void (*cb)(auto (*)(int) -> auto)) {};
auto l3 = [](auto (*cb)(int) -> auto, auto (*d)(auto)) {};
template <class T> void f4(void (*cb)(T, auto));
void f5(int (*cb)(auto (*)(int) -> auto));
void f6(void (*cb)(C auto));
