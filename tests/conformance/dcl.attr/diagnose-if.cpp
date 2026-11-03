// [dcl.attr.grammar]/5 (an implementation attribute) and [expr.call]: clang's
// diagnose_if(cond, "message", "error"|"warning"). A condition naming a
// parameter is evaluated at each call with the converted arguments (default
// arguments included; at the operator of an operator call, the declarator of a
// constructed variable, after the object argument of an explicit object member
// function), in a template only by each instantiation; any other condition at
// each use of the function. The first true "error" is reported alone, else
// every true "warning" in declaration order (redeclarations' accumulate); a
// condition that is not a constant expression is ignored.
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 70:19 warning "neg"
//@ diag: 66:30 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 70:19 warning "neg2"
//@ diag: 68:18 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 70:26 warning "neg"
//@ diag: 66:30 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 70:26 warning "neg2"
//@ diag: 68:18 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 70:26 warning "redecl"
//@ diag: 69:30 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 70:33 error "very neg"
//@ diag: 67:18 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 73:17 warning "dneg"
//@ diag: 72:35 note "from 'diagnose_if' attribute on 'd':"
//@ diag: 73:37 warning "neg"
//@ diag: 66:30 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 73:37 warning "neg2"
//@ diag: 68:18 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 83:20 warning "ctor neg"
//@ diag: 76:27 note "from 'diagnose_if' attribute on 'C':"
//@ diag: 83:29 warning "plus neg"
//@ diag: 77:45 note "from 'diagnose_if' attribute on 'operator+':"
//@ diag: 83:39 warning "call neg"
//@ diag: 78:46 note "from 'diagnose_if' attribute on 'operator()':"
//@ diag: 83:49 warning "static neg"
//@ diag: 79:38 note "from 'diagnose_if' attribute on 's':"
//@ diag: 83:58 warning "static neg"
//@ diag: 79:38 note "from 'diagnose_if' attribute on 's':"
//@ diag: 83:67 warning "eo neg"
//@ diag: 80:45 note "from 'diagnose_if' attribute on 'x':"
//@ diag: 83:75 error "eo big"
//@ diag: 81:52 note "from 'diagnose_if' attribute on 'y':"
//@ diag: 87:19 warning "tneg"
//@ diag: 85:47 note "from 'diagnose_if' attribute on 't<int>':"
//@ diag: 90:31 warning "k used"
//@ diag: 89:25 note "from 'diagnose_if' attribute on 'k':"
//@ diag: 91:25 warning "k used"
//@ diag: 89:25 note "from 'diagnose_if' attribute on 'k':"
//@ diag: 91:28 warning "k used"
//@ diag: 89:25 note "from 'diagnose_if' attribute on 'k':"
//@ diag: 95:23 warning "q3"
//@ diag: 94:30 note "from 'diagnose_if' attribute on 'q':"
//@ diag: 99:21 warning "big T"
//@ diag: 97:45 note "from 'diagnose_if' attribute on 'w<Big>':"
//@ diag: 86:38 warning "neg"
//@ diag: 87:30 note "in instantiation of function template specialization 'u<int>' requested here"
//@ diag: 66:30 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 86:38 warning "neg2"
//@ diag: 68:18 note "from 'diagnose_if' attribute on 'f':"
//@ diag: 86:45 warning "tneg"
//@ diag: 85:47 note "from 'diagnose_if' attribute on 't<int>':"
//@ diag: 86:54 warning "eo neg"
//@ diag: 80:45 note "from 'diagnose_if' attribute on 'x':"
void f(int n) __attribute__((diagnose_if(n < 0, "neg", "warning")))
  __attribute__((diagnose_if(n < -5, "very neg", "error")))
  __attribute__((diagnose_if(n < -1, "neg2", "warning")));
void f(int n) __attribute__((diagnose_if(n == -3, "redecl", "warning")));
void use1() { f(-2); f(-3); f(-7); f(1); }

void d(int n = -1) __attribute__((diagnose_if(n < 0, "dneg", "warning")));
void use2() { d(); (void)sizeof(f(-2), 0); }

struct C {
  C(int n) __attribute__((diagnose_if(n < 0, "ctor neg", "warning")));
  int operator+(int n) const __attribute__((diagnose_if(n < 0, "plus neg", "warning")));
  int operator()(int n) const __attribute__((diagnose_if(n < 0, "call neg", "warning")));
  static int s(int n) __attribute__((diagnose_if(n < 0, "static neg", "warning")));
  void x(this C self, int n) __attribute__((diagnose_if(n < 0, "eo neg", "warning")));
  void y(this const C& self, int n) __attribute__((diagnose_if(n > 1, "eo big", "error")));
};
void use3(C c) { C v(-1); c + -1; c(-1); C::s(-1); c.s(-1); c.x(-1); c.y(2); c.y(1); }

template <class T> void t(T n) __attribute__((diagnose_if(n < 0, "tneg", "warning")));
template <class T> void u(C c) { f(-2); t(-1); c.x(-1); }
void use4() { t(-1); t(1.0); u<int>(C(0)); }

void k() __attribute__((diagnose_if(1, "k used", "warning")));
template <class T> void v() { k(); }
void use5() { auto p = &k; k(); (void)p; }

constexpr int cf(int n) { return n; }
void q(int n) __attribute__((diagnose_if(cf(n) == 3, "q3", "warning")));
void use6(int r) { q(3); q(r); }

template <class T> void w(T) __attribute__((diagnose_if(sizeof(T) > 100, "big T", "warning")));
struct Big { char c[200]; };
void use7() { w(1); w(Big{}); }
