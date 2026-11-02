// [dcl.init.general]/16.6.2.2, [dcl.init.aggr]: a parenthesized aggregate
// initialization initializes the elements in order, the first failure
// ending it, and only then reports excess arguments at the entity. An
// array initializes every argument as an element, past the bound too; a
// union its first member from the first argument (clang
// TryOrBuildParenListInitialization; WP-316: kiln reported the excess
// first).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 26:8 error "array initializer must be an initializer list"
//@ diag: 27:19 error "cannot initialize an array element of type 'int' with an rvalue of type 'int *'"
//@ diag: 28:7 error "excess elements in array initializer"
//@ diag: 29:11 error "cannot initialize a member subobject of type 'int *' with an rvalue of type 'int'"
//@ diag: 30:5 error "excess elements in struct initializer"
//@ diag: 31:8 error "cannot initialize a member subobject of type 'int' with an rvalue of type 'int *'"
//@ diag: 32:8 error "cannot initialize a member subobject of type 'int *' with an rvalue of type 'int'"
//@ diag: 33:5 error "excess elements in union initializer"
//@ diag: 34:5 error "excess elements in union initializer"
struct S { S(int); S(); };
struct B { S arr[2]; };
struct E { int x; int *p; };
union U { int *p; int q; };
union V {};
void f() {
  B b1(S(1), S(2));
  int a1[2](1, 2, (int *)0);
  int a2[1](1, 2);
  E e1(1, 2, 3);
  E e2(1, nullptr, 3);
  E e3((int *)0, 2, 3);
  U u1(1, 2);
  U u2(nullptr, 2);
  V v1(1);
}
