// [class.union.general]/2, /6: the members of an anonymous union member
// of a union are variant members of the enclosing union, so a default
// member initializer inside a nested anonymous union makes the enclosing
// union's default initialization activate it; an assignment through
// another member of the nested union then switches only the nested
// union's active member, and an assignment to a scalar member of the
// enclosing union switches the enclosing one. Found running GCC's C++
// testsuite.
//@ std: c++20
//@ expect: compile-ok
constexpr int inner_then_outer() {
  union {
    union {
      short s{3};
      unsigned char c;
    };
    double d;
  };
  c = 7;
  int r = c;
  d = 1.5;
  return r + static_cast<int>(d * 2);
}
static_assert(inner_then_outer() == 10);

constexpr int inner_only() {
  union {
    union {
      int a{1};
      long b;
    };
    char z;
  };
  b = 40;
  return static_cast<int>(b) + 2;
}
static_assert(inner_only() == 42);
