// [class.copy.elision]/3 (P1825R0): the implicit move selects the
// constructor of the returned class even when it takes an rvalue
// reference to a base, and access is checked on the result rather than
// making the first overload resolution fail, so an inaccessible move
// constructor is an error, not a fallback to the copy constructor.
// clang applies this in every language mode.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17
//@ strict-diags
//@ diag: 30:11 error "calling a private constructor of class 'Shape'"
//@ diag: 23:3 note "declared private here"
//@ diag: 32:10 error "calling a private constructor of class 'Shape'"
//@ diag: 23:3 note "declared private here"
//@ diag: 35:34 error "calling a private constructor of class 'Shape'"
//@ diag: 23:3 note "declared private here"

struct Shape {
  Shape();
  Shape(const Shape &);

private:
  Shape(Shape &&);
};

struct Circle : Shape {};

Shape by_value(Shape s) {
  if (sizeof(s) > 100)
    throw s;
  Circle c;
  return c;
}

Shape by_param(Shape s) { return s; }
