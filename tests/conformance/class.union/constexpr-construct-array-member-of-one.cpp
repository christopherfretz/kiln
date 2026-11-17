// [intro.object]/2, [class.union.general]/6 (note), [expr.const]/5
// (construct_at): constructing an array member of an inactive union member
// as an array of one array (`new (p) T[1]()` for an array `T`, the form a
// `std::construct_at` of an array type takes) creates the member: its
// element is an object of the member's type in the member's storage, so
// the union's active member changes, for an array of int and of class
// type, in a nested union and an anonymous union, and the elements are
// value-initialized. Found running GCC's C++ testsuite.
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 crashes on it (clang 19 rejects it)
#include <new>
namespace std {
template <class T> constexpr T *construct_at(T *p) {
  if constexpr (__is_array(T))
    return ::new ((void *)p) T[1]();
  else
    return ::new ((void *)p) T();
}
} // namespace std
struct S {
  int v;
  constexpr S() : v(7) {}
};
union U {
  long long x = -1;
  int arr[4];
  S sarr[2];
};
constexpr int ints() {
  U u;
  int (*p)[4] = std::construct_at(&u.arr);
  return (*p)[0] + u.arr[3];
}
constexpr int classes() {
  U u;
  std::construct_at(&u.sarr);
  return u.sarr[0].v + u.sarr[1].v;
}
union N {
  long z;
  U inner;
};
constexpr int nested() {
  N n{3};
  std::construct_at(&n.inner);
  std::construct_at(&n.inner.arr);
  return n.inner.arr[1];
}
struct A {
  int k;
  union {
    long l;
    int two[2];
  };
};
constexpr int anonymous() {
  A a{1, {5}};
  std::construct_at(&a.two);
  return a.k + a.two[1];
}
static_assert(ints() == 0);
static_assert(classes() == 14);
static_assert(nested() == 0);
static_assert(anonymous() == 1);
int main() {}
