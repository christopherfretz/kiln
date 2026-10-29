// [over.match.copy]: copy-initialization of a class object from another type
// considers the converting constructors of the target and the conversion
// functions of the source yielding the class or a derived class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:5 error "no viable conversion from 'Q' to 'A'"
//@ diag: 27:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'Q' to 'const A &' for 1st argument"
//@ diag: 27:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'Q' to 'A &&' for 1st argument"
//@ diag: 27:12 note "candidate constructor not viable: no known conversion from 'Q' to 'int' for 1st argument"
//@ diag: 27:40 note "candidate constructor not viable: no known conversion from 'Q' to 'const char *' for 1st argument"
//@ diag: 27:29 note "explicit constructor is not a candidate"
//@ diag: 29:12 note "candidate function"
//@ diag: 38:5 error "no viable conversion from 'E' to 'A'"
//@ diag: 27:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'E' to 'const A &' for 1st argument"
//@ diag: 27:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'E' to 'A &&' for 1st argument"
//@ diag: 27:12 note "candidate constructor not viable: no known conversion from 'E' to 'int' for 1st argument"
//@ diag: 27:40 note "candidate constructor not viable: no known conversion from 'E' to 'const char *' for 1st argument"
//@ diag: 27:29 note "explicit constructor is not a candidate"
//@ diag: 30:21 note "explicit conversion function is not a candidate"
//@ diag: 40:5 error "no viable conversion from 'B' to 'A'"
//@ diag: 27:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'B' to 'const A &' for 1st argument"
//@ diag: 27:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'B' to 'A &&' for 1st argument"
//@ diag: 27:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 27:40 note "candidate constructor not viable: no known conversion from 'B' to 'const char *' for 1st argument"
//@ diag: 27:29 note "explicit constructor is not a candidate"
struct B {};
struct A { A(int); explicit A(double); A(const char *, int = 0); };
struct S { operator A(); };
struct Q { operator B(); };
struct E { explicit operator A(); };
struct D { A a; };
void t(S s, Q q, E e) {
  A a1 = 1;
  A a2 = s;
  A a3 = "x";
  A a4 = 1.0;
  A a5 = q;
  A a6 = e;
  A a7(e);
  A a8 = B();
}
