// [dcl.init.list]/3.1: a designated-initializer-list initializes only an
// aggregate; a class or union with a constructor, a scalar and a reference
// to a non-aggregate are rejected before any constructor (even an
// initializer-list constructor) is considered (clang
// FK_DesignatedInitForNonAggregate).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:3 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 32:3 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 33:3 error "initialization of non-aggregate type 'U' with a designated initializer list"
//@ diag: 34:3 error "initialization of non-aggregate type 'U' with a designated initializer list"
//@ diag: 35:3 error "initialization of non-aggregate type 'L' with a designated initializer list"
//@ diag: 37:3 error "initialization of non-aggregate type 'D' with a designated initializer list"
//@ diag: 38:19 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 39:14 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 40:5 error "initialization of non-aggregate type 'int' with a designated initializer list"
//@ diag: 41:10 error "initialization of non-aggregate type 'const C &' with a designated initializer list"
//@ diag: 43:13 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 44:16 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 45:23 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 47:9 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 48:14 error "initialization of non-aggregate type 'C' with a designated initializer list"
#include <initializer_list>
struct C { C(int); int x; };
union U { U(int); int a; float b; };
struct L { L(std::initializer_list<int>); int x; };
struct A { int p, q; };
struct D { D(A); };
struct E { int m; C c; };

C c1{.x = 1};
C c2 = {.x = 1};
U u1{.a = 1};
U u2 = {.b = 2.0f};
L l1{.x = 1};
D d1{{.p = 1, .q = 2}};
D d2{.p = 1};
E e1{.m = 1, .c = {.x = 2}};
E e2{.m = 1, .c{.x = 2}};
int i{.x = 1};
const C &r = {.x = 1};
const A &ra = {.p = 1};
C arr[1] = {{.x = 1}};
C h() { return {.x = 1}; }
struct M { C c; M() : c{.x = 1} {} };
void g() {
  C t = C{.x = 1};
  C *p = new C{.x = 1};
}
