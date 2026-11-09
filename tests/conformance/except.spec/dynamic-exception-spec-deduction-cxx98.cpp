// C++03 [except.spec]/2, [temp.deduct]/2: before C++11 a function template
// specialization's dynamic exception specification is substituted with its
// declaration once deduction succeeds; an incomplete type is a hard error in
// the specialization's instantiation, not a deduction failure, and drops the
// list (clang `InitFunctionInstantiation`).  WP-1137.
//@ std: c++98
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 34:37 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 36:18 note "in instantiation of function template specialization 'h<Inc>' requested here"
//@ diag: 32:8 note "forward declaration of 'Inc'"
//@ diag: 35:36 error "pointer to incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 36:30 note "in instantiation of function template specialization 'k<Inc>' requested here"
//@ diag: 32:8 note "forward declaration of 'Inc'"
//@ diag: 37:40 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 38:21 note "in instantiation of function template specialization 'a<Inc>' requested here"
//@ diag: 32:8 note "forward declaration of 'Inc'"
//@ diag: 40:31 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 32:8 note "forward declaration of 'Inc'"
//@ diag: 39:37 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 40:17 note "in instantiation of function template specialization 's<Inc>' requested here"
//@ diag: 32:8 note "forward declaration of 'Inc'"
//@ diag: 40:17 error "exception specification in declaration does not match previous declaration"
//@ diag: 40:17 note "previous declaration is here"
//@ diag: 41:76 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 42:18 note "in instantiation of function template specialization 'W<E>::m<Inc>' requested here"
//@ diag: 32:8 note "forward declaration of 'Inc'"
//@ diag: 41:76 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 42:30 note "in instantiation of template class 'W<Inc>' requested here"
//@ diag: 32:8 note "forward declaration of 'Inc'"
struct Inc;
struct E {};
template<class T> void h(T*) throw(T) {}
template<class T> void k() throw(T*) {}
void u(Inc *p) { h(p); h(p); k<Inc>(); }
template<class T> void a(T*) throw(T, E) {}
void (*fp)(Inc*) = &a;
template<class T> void s(T*) throw(T);
template<> void s(Inc*) throw(Inc);
template<class T> struct W { template<class U> static void m(U*) throw(T, U); };
void w(Inc *p) { W<E>::m(p); W<Inc>::m((E*)0); }
template<class T> void ok(T*) throw(T) {}
void v(E *p) { ok(p); }
