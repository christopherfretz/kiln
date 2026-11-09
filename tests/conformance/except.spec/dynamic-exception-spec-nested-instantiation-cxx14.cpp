// C++14 [except.spec]/2, [temp.deduct.general]/11: the types of a dynamic
// exception specification in a function type nested in a substituted type
// (a parameter, a typedef, a member or a local variable) are checked; a
// failure fails the substitution: a deduction failure while deducing, else
// an error at the function type's first token (clang
// `TransformExceptionSpec`).  WP-1137.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 31:23 error "exception specifications are not allowed in typedefs"
//@ diag: 30:10 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 34:8 note "in instantiation of template class 'S<Inc>' requested here"
//@ diag: 28:8 note "forward declaration of 'Inc'"
//@ diag: 31:11 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 28:8 note "forward declaration of 'Inc'"
//@ diag: 32:3 error "pointer to incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 28:8 note "forward declaration of 'Inc'"
//@ diag: 35:37 error "rvalue reference type 'int &&' is not allowed in exception specification"
//@ diag: 36:10 note "in instantiation of template class 'R<int &&>' requested here"
//@ diag: 38:12 error "no matching function for call to 'f'"
//@ diag: 37:24 note "candidate template ignored: substitution failure [with T = Inc]: incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 38:23 error "no matching function for call to 'f'"
//@ diag: 37:24 note "candidate template ignored: substitution failure [with T = int &&]: rvalue reference type 'int &&' is not allowed in exception specification"
//@ diag: 39:30 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 40:15 note "in instantiation of function template specialization 'g<Inc>' requested here"
//@ diag: 28:8 note "forward declaration of 'Inc'"
struct Inc;
template<class T> struct S {
  void m(void (*)() throw(T));
  typedef void (*P)() throw(T);
  void (*q)() throw(T*);
};
S<Inc> s;
template<class T> struct R { void m(void (*)() throw(T)); };
R<int&&> r;
template<class T> void f(void (*)() throw(T)) {}
void u() { f<Inc>(0); f<int&&>(0); f<int>(0); }
template<class T> void g() { void (*p)() throw(T) = 0; (void)p; }
template void g<Inc>();
template void g<int>();
