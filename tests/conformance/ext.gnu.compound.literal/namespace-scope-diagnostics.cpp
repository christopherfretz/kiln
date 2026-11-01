// Namespace-scope compound literals ([GNU]; C99 6.5.2.5p3, clang's
// CheckForConstantInitializer): the initializer must be a constant
// initializer. The culprit is the first non-constant element of an
// aggregate list, the `{` of a scalar or vector literal, or a
// constructor call. A variable template's literal is checked when
// instantiated; a default member initializer is at class scope.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:14 error "initializer element is not a compile-time constant"
//@ diag: 25:14 error "initializer element is not a compile-time constant"
//@ diag: 28:12 error "initializer element is not a compile-time constant"
//@ diag: 31:25 error "initializer element is not a compile-time constant"
//@ diag: 34:8 error "initializer element is not a compile-time constant"
//@ diag: 35:18 error "initializer element is not a compile-time constant"
//@ diag: 36:16 error "initializer element is not a compile-time constant"
//@ diag: 37:11 error "initializer element is not a compile-time constant"
//@ diag: 29:29 error "initializer element is not a compile-time constant"
//@ diag: 30:11 note "in instantiation of variable template specialization 'vt<int>' requested here"
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
