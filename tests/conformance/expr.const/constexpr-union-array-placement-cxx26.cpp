// [intro.object]/2 (P2747R2): an array placement new at the first element
// of an inactive union member of array type creates that member, which
// becomes the active member ([class.union.general]/6 note). Found running
// GCC's C++ testsuite; GCC 16.1 agrees.
//@ skip(clang): clang 22 rejects constructing a subobject of an inactive union member
//@ oracle: gcc
//@ std: c++26
//@ expect: compile-ok
using size_t = decltype(sizeof(0));
constexpr void *operator new[](size_t, void *p) noexcept { return p; }

union U {
  int x;
  int arr[4];
};

constexpr int fill() {
  U u{.x = 1};
  int *p = new (&u.arr[0]) int[4]{10, 20, 30, 40};
  return p[3] + u.arr[1];
}
static_assert(fill() == 60);

struct S {
  char tag;
  union {
    long l;
    short s[3];
  };
};
constexpr int nested() {
  S v{'a', {.l = 5}};
  new (&v.s[0]) short[3]{1, 2, 3};
  v.s[2] += 4;
  return v.s[0] + v.s[1] + v.s[2];
}
static_assert(nested() == 10);
