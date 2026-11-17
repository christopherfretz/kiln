// [dcl.fct.default]/4: within one scope a default argument of an extern "C"
// function is not redefined, even after a redeclaration in another
// namespace gave its own. The accepted side: extern-c-scopes-run.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:36 error "redefinition of default argument"
//@ diag: 8:36 note "previous definition is here"
namespace A { extern "C" int f(int = 5); }
namespace B { extern "C" int f(int = 6); }
namespace A { extern "C" int f(int = 5); }
