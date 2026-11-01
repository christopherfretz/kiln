// [dcl.init.general]/16.6.2.2, /16.6.2.3 (P0960): a parenthesized
// initialization of a union initializes its first member from the single
// expression (a second one is excess); a reference member left without an
// expression is ill-formed. Narrowing is allowed in parentheses, unlike
// in braces. Matches clang 19.
//@ expect: compile-fail
//@ diag: 15:3 error "excess elements in union initializer"
//@ diag: 16:6 error "reference member of type 'int &' uninitialized"
//@ diag: 13:8 note "uninitialized reference member is here"
//@ diag: 20:5 error "type 'double' cannot be narrowed to 'int' in initializer list"
union U { int i; float f; };
struct R { int x;
  int& r; };
struct P { int a; int b; };
U a(1, 2);
R r(1);
U ok(2.0);
P q(1.0, 2);
int arr[2](1.0);
P w{1.5, 2};
