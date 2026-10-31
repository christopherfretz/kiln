// [over.match.ctor]/1, [class.copy.ctor]/5: copying an object of the
// class itself considers every constructor, and a failure lists them;
// a constructor template that would take its own class by value is
// never a candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "no matching constructor for initialization of 'S'"
//@ diag: 20:3 note "candidate constructor not viable: 1st argument ('const S') would lose const qualifier"
//@ diag: 21:22 note "candidate template ignored: instantiation would take its own class type by value"
//@ diag: 19:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 25:3 error "no matching constructor for initialization of 'S'"
//@ diag: 20:3 note "candidate constructor not viable: 1st argument ('const S') would lose const qualifier"
//@ diag: 21:22 note "candidate template ignored: instantiation would take its own class type by value"
//@ diag: 19:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 29:3 error "no matching constructor for initialization of 'B'"
//@ diag: 26:17 note "candidate constructor not viable: no known conversion from 'const D' to 'B &' for 1st argument"
//@ diag: 26:12 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
struct S {
  S();
  S(S&);
  template <class T> S(T);
};
const S c;
S d(c);
S e = c;
struct B { B(); B(B&); };
struct D : B {};
const D cd;
B b(cd);
