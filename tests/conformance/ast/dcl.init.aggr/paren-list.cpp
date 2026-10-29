//@ expect: ast
//@ ast-filter: f
// [dcl.init.general]/16.5, /16.6.2.2 (P0960): parenthesized aggregate
// initialization when no constructor is viable. The elements are
// copy-initialized from the arguments (a base from a prvalue of its class
// by its move constructor); the rest take their default member
// initializers or are value-initialized (a class by its default
// constructor, zeroed unless user-provided); arrays take a filler. A
// functional or explicit cast wraps the list in a no-op cast. The
// variable is `parenlistinit`. Matches clang 19.
struct B { int b; };
struct C { C(); };
struct D { int a; };
struct F { F(); ~F(); };
struct P : B { int x = 5; C c; int z; };
struct Q { int x; D d; F g; };
struct R { int &r; int y; };
struct A { int x, y; };
int arr[3](1, 2);
int all[](1, 2, 3);
void f() {
  P p(B{1}, 2);
  P q(B{1});
  Q k(1);
  int i = 0;
  R r(i);
  (void)A(1, 2);
  (void)(A)1;
  (void)static_cast<A>(1);
  int a2[2](1);
}
