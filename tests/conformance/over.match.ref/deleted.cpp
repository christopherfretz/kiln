// [over.match.ref], [dcl.init.ref]/5: a deleted conversion function
// selected for a reference binding. An initialization diagnoses it
// (naming the referenced type); a candidate's argument conversion
// treats it as no conversion (clang's FindConversionForRefInit).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:15 error "conversion function from 'Del' to 'const long' invokes a deleted function"
//@ diag: 26:3 note "'operator int' has been explicitly marked deleted here"
//@ diag: 39:5 error "conversion function from 'Del' to 'const long' invokes a deleted function"
//@ diag: 26:3 note "'operator int' has been explicitly marked deleted here"
//@ diag: 32:20 note "passing argument to parameter 'l' here"
//@ diag: 40:3 error "no matching function for call to 'zz'"
//@ diag: 33:6 note "candidate function not viable: no known conversion from 'Del' to 'int &' for 1st argument"
//@ diag: 41:8 error "conversion function from 'Del' to 'long' invokes a deleted function"
//@ diag: 26:3 note "'operator int' has been explicitly marked deleted here"
//@ diag: 42:8 error "conversion function from 'E' to 'int' invokes a deleted function"
//@ diag: 30:3 note "'operator int &' has been explicitly marked deleted here"
//@ diag: 43:3 error "no matching function for call to 'zz'"
//@ diag: 33:6 note "candidate function not viable: no known conversion from 'E' to 'int &' for 1st argument"
//@ diag: 44:9 error "conversion function from 'Del' to 'int' invokes a deleted function"
//@ diag: 26:3 note "'operator int' has been explicitly marked deleted here"
//@ diag: 45:7 error "conversion function from 'E' to 'int' invokes a deleted function"
//@ diag: 30:3 note "'operator int &' has been explicitly marked deleted here"
//@ diag: 35:15 note "passing argument to parameter 'l' here"
struct Del {
  operator int() = delete;
  operator int &() const = delete;
};
struct E {
  operator int &() = delete;
};
void z(const long &l);
void zz(int &l);
struct S {
  void m(int &l);
};
void t(Del d, E e, S s) {
  const long &r = d;
  z(d);
  zz(d);
  long v = d;
  int &q = e;
  zz(e);
  int &&rr = d;
  s.m(e);
}
