// [class.access.base]/2: a base-specifier without an access-specifier is
// public in a class defined with `struct` and private in one defined with
// `class`; the class-key of the definition decides, not that of an earlier
// declaration. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags

class A;
struct B { int m; };
struct A : B {};
struct C : A {
  B *g() { return this; }
  int h() { return m; }
};
B *f(A *a) { return a; }

int main() { A a; return f(&a)->m; }
