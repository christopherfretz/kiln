// Class compound literal errors (WP-191): the list-initialization of an
// explicit cast ([over.match.list]; explicit constructors allowed), an
// aggregate's narrowing and excess elements, an abstract class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:9 error "no matching constructor for initialization of 'Y'"
//@ diag: 16:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const Y' for 1st argument"
//@ diag: 16:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'Y' for 1st argument"
//@ diag: 16:12 note "candidate constructor not viable: requires 2 arguments, but 1 was provided"
//@ diag: 22:13 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 22:13 note "insert an explicit cast to silence this issue"
//@ diag: 23:16 error "excess elements in struct initializer"
//@ diag: 24:9 error "allocating an object of abstract class type 'A'"
//@ diag: 18:25 note "unimplemented pure virtual method 'f' in 'A'"
struct X { explicit X(int); int v; };
struct Y { Y(int, int); };
struct Z { int a; };
struct A { virtual void f() = 0; };
void f(double d) {
  (void)(X){1};
  (void)(Y){1};
  (void)(Z){d};
  (void)(Z){1, 2};
  (void)(A){};
}
