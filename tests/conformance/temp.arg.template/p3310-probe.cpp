// P3310R5 ([temp.arg.template], a DR): a template template argument with
// default arguments matches a template template parameter with fewer
// parameters only through the parameter, so overloads on the parameter's
// arity are not ambiguous (clang 19 reports the call as ambiguous).
//@ expect: compile-ok
//@ std: c++26
//@ requires: clang>=22
//@ strict-diags
template <class T, class U = int> struct A {};
template <template <class> class TT> int f(TT<int>) { return 1; }
template <template <class, class> class TT> int f(TT<int, int>) { return 2; }
int g() { return f(A<int>()); }
