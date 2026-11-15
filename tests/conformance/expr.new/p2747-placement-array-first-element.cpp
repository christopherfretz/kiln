// [expr.const.core]/2.20.2.1 (C++26, P2747R2): a placement new of an array
// type T is a constant expression when its pointer points to the first
// element of an object of a type similar to T, so `new (a) int[3]` into an
// `int a[3]` (the pointer is the decayed `&a[0]`) and an array placement new
// into std::allocator<int> storage are valid. GCC 16 accepts both; clang 22
// rejects them ("placement new would change type of storage"), kiln follows
// the standard. Found running GCC's C++ testsuite (WP-1464).
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 22 rejects an array placement new at the first element of an array object
#include <memory>
#include <new>

constexpr int local_array() {
  int a[3] = {};
  int *q = new (a) int[3]{1, 2, 3};
  return q[2] + a[0];
}
static_assert(local_array() == 4);

constexpr int allocator_storage() {
  std::allocator<int> al;
  int *p = al.allocate(4);
  int *q = ::new (p) int[4]{1, 2, 3, 4};
  int s = q[3] + p[0];
  al.deallocate(p, 4);
  return s;
}
static_assert(allocator_storage() == 5);
