// Function templates whose signature names a closure type, shared by
// ../lambda-in-signature-linkage-two-units-run.cpp and
// lambda-in-signature-linkage-other.cpp. Each closure's call operator
// returns the address of its own static local, so one definition across
// the program means one address.
template <class T> auto ret(T) -> decltype([] { static int x; return &x; }) { return {}; }
template <class T> int *par(T, decltype([] { static int x; return &x; }) f = {}) { return f(); }
template <class T, decltype([] { static int x; return &x; }) V = {}> int *nttp(T) { return V(); }
template <class T> struct S {
  static auto nret(T) -> decltype([] { static int x; return &x; }) { return {}; }
};
namespace ns {
template <class T> auto ret(T) -> decltype([] { static int x; return &x; }) { return {}; }
template <class T, class U = decltype([] { static int x; return &x; })> int *dta(T) { return U{}(); }
}
// Every use, in the same order in both units: the closures are numbered in
// order of substitution ([expr.prim.lambda.closure]/2).
#define USES(X)                                                                \
  X(ret(1)()) X(ret(1L)()) X(par(1)) X(nttp(1)) X(S<int>::nret(1)())             \
  X(ns::ret(1)()) X(ns::dta(1))
