// [dcl.fct.def.delete]/2, [temp.arg.explicit]/3, [expr.const]/18: a
// constant subexpression completes (and is evaluated) while the enclosing
// expression is still being built. A deleted function the enclosing
// expression names is decided by that expression, once: a call to it
// is "call to deleted function", a use that is not called is "attempt
// to use"; evaluating the explicit argument of f<1> or X<f<1>()> does not
// report it early (WP-1170).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "call to deleted function 'del'"
//@ diag: 22:6 note "candidate function has been explicitly deleted"
//@ diag: 26:22 error "call to deleted function 'del'"
//@ diag: 22:6 note "candidate function has been explicitly deleted"
//@ diag: 27:21 error "attempt to use a deleted function"
//@ diag: 22:6 note "'del' has been explicitly marked deleted here"
//@ diag: 28:24 error "call to deleted function 'tdel'"
//@ diag: 23:25 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 29:30 error "attempt to use a deleted function"
//@ diag: 23:25 note "'tdel<int>' has been explicitly marked deleted here"
template <int N> struct X { static constexpr int v = N; };
template <int N> constexpr int f() { return N; }
void del(int) = delete;
template <class T> void tdel(T) = delete;
void take2(void (*)(int), int);
void call() { del(X<f<1>()>::v); }
void call_nested() { del(f<f<2>()>()); }
void name() { take2(del, X<f<3>()>::v); }
void call_template() { tdel<int>(X<f<4>()>::v); }
void name_template() { take2(tdel<int>, f<f<5>()>()); }
