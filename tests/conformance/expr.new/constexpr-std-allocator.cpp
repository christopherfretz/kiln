// [allocator.members], [expr.const]/5.20: within std::allocator<T>::allocate,
// `operator new(n)` allocates T[n / sizeof(T)] whose elements are outside
// their lifetime until constructed (std::construct_at). Elsewhere the call
// is not constant; the storage must be deallocated within the evaluation;
// the byte count must be a multiple of sizeof(T). The std facilities are
// hand-written so that the notes land in this file (clang recognizes them by
// name). spec/oven.md "Transient allocation".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:13 note "cannot allocate untyped memory in a constant expression; use 'std::allocator<T>::allocate' to allocate memory of type 'T'"
//@ diag: 73:15 note "in call to 'untyped()'"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:61 note "allocation performed here was not deallocated"
//@ diag: 89:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:69 note "allocated size 5 is not a multiple of size 2 of element type 'short'"
//@ diag: 86:3 note "in call to 'a.allocate(2)'"
//@ diag: 89:15 note "in call to 'bad_size()'"
//@ diag: 99:15 error "static assertion expression is not an integral constant expression"
//@ diag: 95:11 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 99:15 note "in call to 'read_unconstructed()'"
//@ diag: 109:15 error "static assertion expression is not an integral constant expression"
//@ diag: 107:10 note "read of heap allocated object that has been deleted"
//@ diag: 109:15 note "in call to 'use_after_free()'"
//@ requires: clang<22
// clang 19 spelling; constexpr-std-allocator-clang22.cpp has clang 22's (WP-394).
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T> struct allocator {
  constexpr T *allocate(size_t n) { return static_cast<T *>(operator new(n * sizeof(T))); }
  constexpr void deallocate(T *p, size_t) { operator delete(p); }
};
// An allocator whose byte count is not a multiple of the element size.
template <> struct allocator<short> {
  constexpr short *allocate(size_t n) { return static_cast<short *>(operator new(n * 2 + 1)); }
};
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

// Allocate, construct, read, destroy, deallocate: a constant expression.
constexpr int sum(int n) {
  std::allocator<int> a;
  int *p = a.allocate(n);
  for (int i = 0; i < n; ++i)
    std::construct_at(p + i, i);
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += p[i];
  for (int i = 0; i < n; ++i)
    std::destroy_at(p + i);
  a.deallocate(p, n);
  return s;
}
static_assert(sum(100) == 4950);

// An empty allocation.
constexpr bool empty() {
  std::allocator<long> a;
  long *p = a.allocate(0);
  a.deallocate(p, 0);
  return true;
}
static_assert(empty());

// Untyped allocation outside std::allocator<T>::allocate.
constexpr int untyped() {
  void *p = operator new(4);
  operator delete(p);
  return 1;
}
static_assert(untyped());

// Never deallocated.
constexpr int leak() {
  std::allocator<int> a;
  a.allocate(2);
  return 1;
}
static_assert(leak());

// The byte count must be a multiple of sizeof(T).
constexpr int bad_size() {
  std::allocator<short> a;
  a.allocate(2);
  return 1;
}
static_assert(bad_size());

// The elements of the storage are outside their lifetime until constructed.
constexpr int read_unconstructed() {
  std::allocator<int> a;
  int *p = a.allocate(2);
  int r = p[0];
  a.deallocate(p, 2);
  return r;
}
static_assert(read_unconstructed());

// Use after deallocate.
constexpr int use_after_free() {
  std::allocator<int> a;
  int *p = a.allocate(1);
  std::construct_at(p, 1);
  a.deallocate(p, 1);
  return *p;
}
static_assert(use_after_free());
