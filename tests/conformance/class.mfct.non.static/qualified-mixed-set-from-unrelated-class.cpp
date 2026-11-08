// [class.mfct.non.static]/2 [over.match.call.general]/3: `F::name(...)`
// inside a member of a class not derived from `F`, naming a set that mixes
// static and non-static members, is an implicit member access through
// `(*this)`: the call resolves against the object, so a non-static winner
// is clang's "no matching member function" with a candidate note per
// member (an object-argument mismatch names a member template, not its
// specialization), and a static winner is valid. spec/sema-overload.md §2.3,
// WP-788 (WP-823).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:23 error "no matching member function for call to 'isT'"
//@ diag: 35:20 note "in instantiation of member function 'B<Other>::h' requested here"
//@ diag: 20:7 note "candidate function not viable: no known conversion from 'B<Other>' to 'const F' for object argument"
//@ diag: 21:14 note "candidate function not viable: requires single argument 'i', but no arguments were provided"
//@ diag: 31:23 error "no matching member function for call to 'tm'"
//@ diag: 36:20 note "in instantiation of member function 'B<Other>::q' requested here"
//@ diag: 23:26 note "candidate function template not viable: no known conversion from 'B<Other>' to 'F' for object argument"
//@ diag: 22:33 note "candidate function template not viable: requires 1 argument, but 0 were provided"
struct F {
  int isT() const { return 1; }
  static int isT(int i) { return i; }
  template <class U> static int tm(U) { return 2; }
  template <class U> int tm() { return 3; }
};
template <class T> struct B : T {
  int h() { return F::isT(); }
  int k() { return F::isT(1); }
  int m() { int (*p)(int) = F::isT; return p(2); }
  int n() { int (*p)(int) = &F::isT; return p(2); }
  int o() { return F::tm(1) + F::tm<int>(2); }
  int q() { return F::tm<int>(); }
};
struct Other {};
int x = B<Other>().k() + B<Other>().m() + B<Other>().n() + B<Other>().o();
int y = B<Other>().h();
int z = B<Other>().q();
