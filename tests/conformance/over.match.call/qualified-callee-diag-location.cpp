// [over.match.call]/1, [over.match.viable]/1: 'no matching function' for a qualified callee
// (namespace, class, class template specialization, global, template-id) is located at the
// start of its nested-name-specifier, as clang's OverloadExpr begin location
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:3 error "no matching function for call to 'f'"
//@ diag: 28:20 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 28:38 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 33:3 error "no matching function for call to 'g'"
//@ diag: 28:76 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'char')"
//@ diag: 34:3 error "no matching function for call to 'f'"
//@ diag: 29:24 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 29:49 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 35:3 error "no matching function for call to 'g'"
//@ diag: 29:94 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'char')"
//@ diag: 36:3 error "no matching function for call to 'f'"
//@ diag: 30:42 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 30:67 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 37:3 error "no matching function for call to 'g'"
//@ diag: 30:112 note "candidate template ignored: deduced conflicting types for parameter 'U' ('int' vs. 'char')"
//@ diag: 38:3 error "no matching function for call to 'f'"
//@ diag: 28:20 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 28:38 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 39:3 error "no matching function for call to 'g'"
//@ diag: 28:76 note "candidate function template not viable: requires 2 arguments, but 3 were provided"
//@ diag: 40:3 error "no matching function for call to 'g'"
//@ diag: 30:112 note "candidate function template not viable: requires 2 arguments, but 3 were provided"
namespace N { void f(int, int); void f(char*, int); template<class T> void g(T, T); }
struct S { static void f(int, int); static void f(char*, int); template<class T> static void g(T, T); };
template<class T> struct X { static void f(int, int); static void f(char*, int); template<class U> static void g(U, U); };
void t() {
  N::f();
  N::g(1, 'a');
  S::f();
  S::g(1, 'a');
  X<int>::f();
  X<int>::g(1, 'a');
  ::N::f();
  N::g<int>(1, 2, 3);
  X<int>::template g<int>(1, 2, 3);
}
