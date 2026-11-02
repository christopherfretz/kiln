// [temp.names]/3 (P0846): an unqualified member function name followed by
// '<' inside its class is assumed to name a template; no function
// template is found. clang 19 diagnoses the member as found by the class
// lookup: "member 'f' used before its declaration" when the class
// declares it, "explicit qualification required to use member 'bf' from
// dependent base class" when it inherits it, with a note per
// declaration, and from a static member function the call has no object.
// clang 22's wording is args-for-non-template-member.cpp.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 33:11 error "member 'f' used before its declaration"
//@ diag: 30:7 note "member is declared here"
//@ diag: 31:20 error "member 'f' used before its declaration"
//@ diag: 30:7 note "member is declared here"
//@ diag: 31:32 error "explicit qualification required to use member 'bf' from dependent base class"
//@ diag: 28:16 note "member is declared here"
//@ diag: 31:45 error "member 'f0' used before its declaration"
//@ diag: 30:19 note "member is declared here"
//@ diag: 32:27 error "member 'f' used before its declaration"
//@ diag: 30:7 note "member is declared here"
//@ diag: 32:27 error "call to non-static member function without an object argument"
//@ diag: 35:63 error "member 'f' used before its declaration"
//@ diag: 35:38 note "member is declared here"
//@ diag: 35:75 error "explicit qualification required to use member 'bf' from dependent base class"
//@ diag: 28:16 note "member is declared here"
struct B { int bf(int); };
struct E : B {
  int f(int); int f0();
  int k() { return f<int>(1) + bf<int>(1) + f0<int>(); }
  static int s() { return f<int>(1); }
  int m = f<int>(1);
};
template<class T> struct D : B { int f(int); int k() { return f<int>(1) + bf<int>(1); } };
