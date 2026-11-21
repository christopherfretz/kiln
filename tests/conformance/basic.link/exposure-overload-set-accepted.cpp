// [basic.link]/14.5: only an id-expression names the functions of its
// overload set. These do not name a TU-local function in an exposure:
// an operator expression (its candidates are not found by an
// id-expression), the body of a function that is not inline and the
// initializer of a variable that is not inline (/15.1, /15.2), a
// specialization of an inline template whose call finds no TU-local
// function (in the definition or by argument-dependent lookup), a
// specialization of a template that is not inline (/15.1 ignores its
// body, so it is not TU-local) and a TU-local function's body. GCC 16.1
// and clang 22 accept each too. The exposures are in
// exposure-overload-set.cpp; an importer of these forms is
// module.interface/exposure-overload-set-importer-run.cpp.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
export module M;
static int f(int) { return 1; }
int f(double) { return 2; }
namespace N {
struct A {};
int adl(A) { return 3; }
static int adl(int) { return 4; }
}
int adl(double) { return 5; }
struct B {};
static B operator+(B, int) { return {}; }
B operator+(B, B) { return {}; }
template <class T> inline int calls_adl(T x) { return adl(x); }
template <class T> int not_inline(T) { return f(1.0); }
static inline int tu_local() { return f(1.0) + adl(N::A{}); }

export inline B operator_expression() { return B{} + B{}; }
export int not_inline_body() { return f(1.0) + adl(N::A{}) + tu_local(); }
export int variable = f(1.0);
export inline int instantiation() { return calls_adl(0.5); }
export inline int non_inline_template() { return not_inline(1); }
