// [over.match.ctor]/1, [over.ics.rank]/3.3: the candidates of a
// direct-initialization from another class are the constructors. A copy
// constructor reached through 'operator T&' (or 'operator const T&') and a
// move constructor reached through 'operator T' use different conversion
// functions, so neither is better and the call is ambiguous, also from
// C++17. A conversion function yielding the prvalue 'T' whose implicit
// object conversion is no better than the other one's does not resolve
// that (CWG2327 is open); 'B' is the shape of the GCC case, where the
// conversion function yielding 'B' has the worse object conversion.
// clang 22 lets 'operator T' initialize the object directly; the draft's
// text and GCC 16 decide. Found running GCC's C++ testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 22 accepts through its CWG2327 workaround; the text and GCC 16 reject
//@ diag: 42:5 error "call to constructor of 'A' is ambiguous"
//@ diag: 25:8 note "candidate constructor (the implicit copy constructor)"
//@ diag: 25:8 note "candidate constructor (the implicit move constructor)"
//@ diag: 43:5 error "call to constructor of 'B' is ambiguous"
//@ diag: 30:8 note "candidate constructor (the implicit copy constructor)"
//@ diag: 30:8 note "candidate constructor (the implicit move constructor)"
//@ diag: 44:5 error "call to constructor of 'C' is ambiguous"
//@ diag: 36:8 note "candidate constructor (the implicit copy constructor)"
//@ diag: 36:8 note "candidate constructor (the implicit move constructor)"
struct A { int n; };
struct ToA {
  operator A() const;
  operator const A &() const;
};
struct B { int n; };
struct ToB {
  operator B() const;
  operator B &();
  operator const B &() = delete;
};
struct C { C(int); };
struct ToC {
  operator C();
  operator C &();
};
void f(ToA ta, ToB tb, ToC tc) {
  A a(ta);
  B b(tb);
  C c(tc);
  (void)a; (void)b; (void)c;
}
