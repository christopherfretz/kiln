// [class.copy.elision]/3 (C++20, P1825R0): returning a local of a derived
// class moves through the base's move constructor, which is checked for
// access on the result.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++20
//@ strict-diags
//@ diag: 25:10 error "calling a private constructor of class 'Shape'"
//@ diag: 18:3 note "declared private here"
//@ diag: 36:10 error "call to deleted constructor of 'Open'"
//@ diag: 31:3 note "'Open' has been explicitly marked deleted here"

struct Shape {
  Shape();
  Shape(const Shape &);

private:
  Shape(Shape &&);
};

struct Circle : Shape {};

Shape from_derived() {
  Circle c;
  return c;
}

struct Open {
  Open();
  Open(const Open &);
  Open(Open &&) = delete;
};

Open from_deleted() {
  Open o;
  return o;
}
