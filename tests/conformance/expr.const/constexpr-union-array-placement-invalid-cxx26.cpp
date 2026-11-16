// [intro.object]/2: after an array placement new creates the union's
// array member, the previously active member is no longer active.
//@ skip(clang): clang 22 rejects constructing a subobject of an inactive union member
//@ oracle: gcc
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:10 note "read of member 'x' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 24:15 note "in call to 'stale()'"
using size_t = decltype(sizeof(0));
constexpr void *operator new[](size_t, void *p) noexcept { return p; }

union U {
  int x;
  int arr[4];
};

constexpr int stale() {
  U u{.x = 1};
  new (&u.arr[0]) int[4]{10, 20, 30, 40};
  return u.x;
}
static_assert(stale() == 1);
