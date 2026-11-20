// [temp.names]/6, [temp.res.general]/4: a template-id after `template`
// in an expression (`T::template X<int>()`) is assumed to name a function
// or variable template; one that names a class or alias template once
// instantiated is ill-formed (clang 22: "is expected to be a non-type
// template", then the call of the template-id's type).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:49 error "'A::template X' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 29:12 note "in instantiation of function template specialization 'f<A>' requested here"
//@ diag: 21:29 note "class template declared here"
//@ diag: 25:37 error "called object type 'A::template X<int>' is not a function or function pointer"
//@ diag: 26:49 error "'A::template X' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 29:20 note "in instantiation of function template specialization 'g<A>' requested here"
//@ diag: 21:29 note "class template declared here"
//@ diag: 26:37 error "called object type 'A::template X<int>' is not a function or function pointer"
//@ diag: 27:49 error "'A::template Y' is expected to be a non-type template, but instantiated to a type alias template"
//@ diag: 29:28 note "in instantiation of function template specialization 'h<A>' requested here"
//@ diag: 22:22 note "type alias template declared here"
struct A {
  template <class U> struct X { X() {} X(int) {} };
  template <class U> using Y = X<U>;
  template <class U> static int f(U) { return 0; }
};
template <class T> void f() { (void)T::template X<int>(); }
template <class T> void g() { (void)T::template X<int>(1); }
template <class T> void h() { (void)T::template Y<int>; }
template <class T> int k() { return T::template f<int>(1); }
void u() { f<A>(); g<A>(); h<A>(); k<A>(); }
