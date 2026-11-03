// [allocator.members], [expr.const]/5.20: `operator delete(p)` within
// std::allocator<T>::deallocate frees storage std::allocator<T>::allocate
// obtained: a new-expression's allocation, a subobject, a freed pointer, a
// non-heap pointer and a null pointer are not constant, nor is a
// delete-expression of the allocation or the call outside deallocate.
// spec/oven.md "Transient allocation".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:3 note "'delete' used to delete pointer to object allocated with 'std::allocator<...>::allocate'"
//@ diag: 56:15 note "in call to 'delete_allocation()'"
//@ diag: 43:61 note "heap allocation performed here"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:45 note "'std::allocator<...>::deallocate' used to delete pointer to object allocated with 'new'"
//@ diag: 61:3 note "in call to 'a.deallocate(&{*new int[2]#0}[0], 2)'"
//@ diag: 64:15 note "in call to 'deallocate_new()'"
//@ diag: 47:38 note "heap allocation performed here"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:45 note "delete of pointer to subobject '&{*new int[2]#0}[1]' "
//@ diag: 70:3 note "in call to 'a.deallocate(&{*new int[2]#0}[1], 1)'"
//@ diag: 73:15 note "in call to 'deallocate_subobject()'"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:45 note "delete of pointer that has already been deleted"
//@ diag: 80:3 note "in call to 'a.deallocate(&{*new int[2]#0}[0], 2)'"
//@ diag: 83:15 note "in call to 'deallocate_twice()'"
//@ diag: 92:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:45 note "delete of pointer '&x' that does not point to a heap-allocated object"
//@ diag: 89:3 note "in call to 'a.deallocate(&x, 1)'"
//@ diag: 92:15 note "in call to 'deallocate_local()'"
//@ diag: 88:7 note "declared here"
//@ diag: 100:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:45 note "'std::allocator<...>::deallocate' used to delete a null pointer"
//@ diag: 97:3 note "in call to 'a.deallocate(nullptr, 1)'"
//@ diag: 100:15 note "in call to 'deallocate_null()'"
//@ diag: 109:15 error "static assertion expression is not an integral constant expression"
//@ diag: 106:3 note "subexpression not valid in a constant expression"
//@ diag: 109:15 note "in call to 'untyped_delete()'"
//@ requires: clang<22
// clang 19 spelling; constexpr-std-deallocate-clang22.cpp has clang 22's (WP-394).
using size_t = decltype(sizeof(0));
namespace std {
template <class T> struct allocator {
  constexpr T *allocate(size_t n) { return static_cast<T *>(operator new(n * sizeof(T))); }
  constexpr void deallocate(T *p, size_t) { operator delete(p); }
};
} // namespace std
constexpr int *make_array() { return new int[2]; }

// A std::allocator allocation freed by a delete-expression.
constexpr int delete_allocation() {
  std::allocator<int> a;
  int *p = a.allocate(2);
  delete[] p;
  return 1;
}
static_assert(delete_allocation());

// A new-expression's allocation freed by deallocate.
constexpr int deallocate_new() {
  std::allocator<int> a;
  a.deallocate(make_array(), 2);
  return 1;
}
static_assert(deallocate_new());

// Not the start of the allocation.
constexpr int deallocate_subobject() {
  std::allocator<int> a;
  int *p = a.allocate(2);
  a.deallocate(p + 1, 1);
  return 1;
}
static_assert(deallocate_subobject());

// Twice.
constexpr int deallocate_twice() {
  std::allocator<int> a;
  int *p = a.allocate(2);
  a.deallocate(p, 2);
  a.deallocate(p, 2);
  return 1;
}
static_assert(deallocate_twice());

// Not heap storage.
constexpr int deallocate_local() {
  std::allocator<int> a;
  int x = 0;
  a.deallocate(&x, 1);
  return 1;
}
static_assert(deallocate_local());

// A null pointer.
constexpr int deallocate_null() {
  std::allocator<int> a;
  a.deallocate(nullptr, 1);
  return 1;
}
static_assert(deallocate_null());

// `operator delete` outside std::allocator<T>::deallocate.
constexpr int untyped_delete() {
  std::allocator<int> a;
  int *p = a.allocate(1);
  operator delete(p);
  return 1;
}
static_assert(untyped_delete());
