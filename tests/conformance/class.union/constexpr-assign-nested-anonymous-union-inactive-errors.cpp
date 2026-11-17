// [class.union.general]/6: a nested anonymous union whose member has a
// default member initializer has a non-trivial default constructor, so
// an assignment through one of its members does not begin its lifetime:
// after the enclosing union's scalar member became active, the
// assignment is not a constant expression (clang 22 and GCC 16.1 reject it).
//@ std: c++20
//@ expect: compile-fail
constexpr int outer_then_inner() {
  union {
    union {
      short s{3};
      unsigned char c;
    };
    double d;
  };
  d = 1.5;
  c = 7;
  return c;
}
static_assert(outer_then_inner() == 7);
