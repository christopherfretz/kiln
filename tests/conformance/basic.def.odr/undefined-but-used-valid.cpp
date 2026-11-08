// [basic.def.odr]/11, [basic.link]/8: no diagnostic for an undefined
// entity that another translation unit can define or that is not
// odr-used: a specialization whose closure or local class argument comes
// from an inline function, an inline variable or a function template
// instantiation (it takes their linkage); an unnamed class with a typedef
// name for linkage; an `extern "C"` function; a use in an unevaluated
// operand or an uninstantiated template; a static data member with an
// in-class initializer; a pure virtual function called virtually; and an
// internal entity defined after its use. Matches clang 19 and 22.
//@ expect: compile-ok
//@ strict-diags
namespace { struct A {}; struct P { virtual void f() = 0; }; }
template <class F> void g(F);
inline auto lam = [] {};
typedef struct {} T;
extern "C" void ec(A);
struct S { static const int ci = 1; };
static void later();
inline void i1() { g([] {}); struct L {}; g(L{}); }
template <class T> void t1() { g([] {}); }
template <class T> void t2() { g(A{}); }
void h1() { i1(); t1<int>(); g(lam); g(T{}); ec(A{}); }
int h2() { return sizeof(g(A{}), 0) + S::ci; }
void h3(P &p) { p.f(); later(); }
static void later() {}
