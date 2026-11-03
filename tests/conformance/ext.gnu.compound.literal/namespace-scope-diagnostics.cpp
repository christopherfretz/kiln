// Namespace-scope compound literals ([GNU]; C99 6.5.2.5p3, clang's
// CheckForConstantInitializer) at clang 22: the initializer must be a
// constant initializer. The culprit is the first non-constant element,
// looking into the list of a scalar, vector or complex literal too, at
// its expression location (an operator, else where it begins); or a
// constructor call. A literal of dependent type whose elements are not
// dependent is checked at the template definition, else when
// instantiated; a default member initializer is at class scope.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 33:15 error "initializer element is not a compile-time constant"
//@ diag: 34:14 error "initializer element is not a compile-time constant"
//@ diag: 37:16 error "initializer element is not a compile-time constant"
//@ diag: 38:30 error "initializer element is not a compile-time constant"
//@ diag: 40:26 error "initializer element is not a compile-time constant"
//@ diag: 43:8 error "initializer element is not a compile-time constant"
//@ diag: 44:18 error "initializer element is not a compile-time constant"
//@ diag: 45:14 error "initializer element is not a compile-time constant"
//@ diag: 46:11 error "initializer element is not a compile-time constant"
//@ diag: 47:14 error "initializer element is not a compile-time constant"
//@ diag: 48:14 error "initializer element is not a compile-time constant"
//@ diag: 49:15 error "initializer element is not a compile-time constant"
//@ diag: 50:16 error "initializer element is not a compile-time constant"
//@ diag: 51:34 error "initializer element is not a compile-time constant"
//@ diag: 52:31 error "initializer element is not a compile-time constant"
//@ diag: 53:12 note "in instantiation of variable template specialization 'vt2<int>' requested here"
struct P { int a, b; };
int g();
int x = (int){5};
P p = (P){1, 2};
constexpr int cx = (int){7};
int y = (int){g()};
P q = (P){3, g()};
typedef int v4 __attribute__((vector_size(16)));
v4 v = (v4){1, 2, 3, 4};
v4 w = (v4){1, g(), 3, 4};
template<class T> T vt = (T){g()};
int use = vt<int>;
struct Q { int m = (int){g()}; };
struct C { constexpr C(int i) : i(i) {} int i; };
C c = (C){4};
C d = (C){g()};
P r = (P){1, g() + 1};
P s = (P){1, p.a};
P t = (P){x ? 1 : 2, 0};
P u = (P){1, (p.a)};
P z = (P){1, (int)x};
P a2 = (P){1, x = 3};
P b2 = (P){1, x++};
_Complex int ci = (_Complex int){g()};
template<class T> T vt2 = (T){T(g())};
int use2 = vt2<int>;
