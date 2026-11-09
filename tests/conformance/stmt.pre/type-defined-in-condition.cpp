// [stmt.pre]/2, [dcl.type.general]/3: a condition declaration shall not
// define a class or enumeration. The for-init-statement is a simple
// declaration and may; the for condition may not. A range declaration gets
// clang's separate "for range declaration" error.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:24 error "'S1' cannot be defined in a condition"
//@ diag: 16:28 error "'S2' cannot be defined in a condition"
//@ diag: 17:27 error "'S3' cannot be defined in a condition"
//@ diag: 18:52 error "'S5' cannot be defined in a condition"
//@ diag: 19:18 error "types may not be defined in a for range declaration"
//@ diag: 20:22 error "'E1' cannot be defined in a condition"
//@ diag: 21:35 error "'S7' cannot be defined in a condition"
void f1() { if (struct S1 {} *p = nullptr) {} }
void f2() { switch (struct S2 { operator int(); } s = {}) {} }
void f3() { while (struct S3 {} *p = 0) {} }
void f4() { for (struct S4 {} *p = nullptr; struct S5 {} *q = nullptr;) {} }
void f5() { for (struct S6 {int a;} x : (struct S6 [1]){}) {} }
void f6() { if (enum E1 { e } v = e) {} }
void f7() { if (int i = 0; struct S7 {} *p = nullptr) {} }
