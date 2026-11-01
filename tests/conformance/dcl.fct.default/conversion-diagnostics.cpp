// [dcl.fct.default]/5 (WP-156): a default argument copy-initializes the
// parameter; a failure is reported at the parameter with the "passing
// argument to parameter" note. In a copy-initialization from a non-class
// expression only converting constructors are candidates
// ([over.match.copy]/1.1), so the default constructor is not noted.
//@ expect: compile-fail
//@ diag: 19:10 error "no viable conversion from 'int' to 'B'"
//@ diag: 18:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B &' for 1st argument"
//@ diag: 18:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B &&' for 1st argument"
//@ diag: 19:10 note "passing argument to parameter 'x' here"
//@ diag: 21:12 error "no viable conversion from 'int' to 'B'"
//@ diag: 18:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B &' for 1st argument"
//@ diag: 18:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B &&' for 1st argument"
//@ diag: 21:12 note "passing argument to parameter 'y' here"
//@ diag: 23:14 error "no viable conversion from 'int' to 'B'"
//@ diag: 18:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B &' for 1st argument"
//@ diag: 18:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B &&' for 1st argument"
struct B { };
void f(B x = 1);
struct C {
  void g(B y = 2);
};
void h() { B b = 3; }
