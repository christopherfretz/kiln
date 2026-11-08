// [basic.def.odr]/8, [dcl.fct.default]/5: a default argument is
// potentially evaluated only where a call uses it, so the functions it
// names are odr-used, and diagnosed as undefined, only then: `never` and
// `T::T` are not, `used` is, with the note at the use in the argument.
// `~P<int>` is not inline, so it may be defined in another translation
// unit. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Werror=undefined-internal
//@ diag: 14:5 error "function '(anonymous namespace)::used' has internal linkage but is not defined"
//@ diag: 18:14 note "used here"
namespace {
int never();
int used();
struct T { T(int); };
}
void f(int = never());
void g(int = used());
void h(T = 0);
void k() { g(); }
template <class U> struct P { P(int) {} ~P(); };
struct Never;
void d(P<Never> = 0);
void e(P<int> = 0);
void m() { e(); }
