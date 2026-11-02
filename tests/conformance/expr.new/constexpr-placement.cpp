// [expr.const]/5.18 (C++26, P2747R2): only the reserved non-allocating
// placement 'operator new' of <new> may be evaluated (clang 22's
// VisitCXXNewExpr); an alignment argument, a user placement allocation
// function (even a constexpr one) are not supported in a constant
// expression, and an array placement new into std::allocator<T> storage,
// whose objects are elements of T, would change the type of the storage.
// constexpr-placement-ok.cpp holds the valid forms.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ args: -stdlib=libc++
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:12 note "this placement new expression is not supported in constant expressions "
//@ diag: 31:15 note "in call to 'aligned()'"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:3 note "this placement new expression is not supported in constant expressions "
//@ diag: 38:15 note "in call to 'user_placement()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:12 note "placement new would change type of storage from 'int' to 'int[4]'"
//@ diag: 48:15 note "in call to 'array_into_allocator()'"
#include <memory>
#include <new>

struct Tag {};
constexpr void *operator new(std::size_t, Tag, void *p) noexcept { return p; }

constexpr int aligned() {
  int *p = new (std::align_val_t(16)) int;
  return 1;
}
static_assert(aligned());

constexpr int user_placement() {
  int x = 0;
  new (Tag{}, &x) int(1);
  return x;
}
static_assert(user_placement() == 1);

constexpr int array_into_allocator() {
  std::allocator<int> al;
  int *p = al.allocate(4);
  int *q = ::new (p) int[4]{1, 2, 3, 4};
  int s = q[3];
  al.deallocate(p, 4);
  return s;
}
static_assert(array_into_allocator() == 4);
