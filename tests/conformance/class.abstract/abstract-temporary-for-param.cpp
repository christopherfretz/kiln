// [class.abstract]/3: no object of an abstract class can be created except
// as a base subobject; '{}' for an 'A&&' parameter would create one. Found
// by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, an abstract temporary from list-initialization is not rejected
//@ expect: compile-fail
//@ diag: 12:17 error "allocating an object of abstract class type 'A'"
struct A {
  virtual void f() = 0;
};
void take(A &&);
void g() { take({}); }
