// Per-specialization entries (spec/ast.md §18.9.2, §18.9.4): an
// unqualified call in a constraint whose argument depends on a template
// parameter is a dependent call ([temp.dep.general]/2) that satisfaction
// substitutes ([temp.constr.atomic]/3) with the template as its context.
// Argument-dependent lookup ([temp.dep.candidate]/1) finds `lib::make`,
// and the call is resolved there, so its row is recorded keyed by the
// constrained declaration, as a qualified call's (`lib::Y::s()`) is: in a
// requires-clause (`f`), in a nested requirement ([expr.prim.req.nested],
// `g`) and in a constraint checked with a definition (`k`). Reduced from
// real code.
//@ written-refs: inst-adl-constraint.refs
namespace lib {
struct X {};
struct Y { static long s(); };
long make(X);
} // namespace lib

template <class U> concept Big = sizeof(U) >= 4;

template <class T> requires Big<decltype(make(T()))> int f(T);
int a = sizeof(f(lib::X()));

template <class T> int g(T) requires requires { requires Big<decltype(make(T()))>; };
int b = sizeof(g(lib::X()));

template <class T> requires Big<decltype(make(T()))> int k(T) { return 0; }
int c = k(lib::X());

template <class T> requires Big<decltype(T::s())> int q(T);
int d = sizeof(q(lib::Y()));
