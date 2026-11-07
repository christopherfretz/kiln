// [obj.lifetime], [expr.const] (P3726R2, C++26): std::start_lifetime
// initializes nothing, so reading an element of the array it started is
// a read of an uninitialized object; an object whose containing object
// (a union member) is not within its lifetime cannot have its lifetime
// started (GCC: "containing object not within lifetime"; kiln names the
// inactive member).
//@ skip(clang): clang 22 has neither __builtin_start_lifetime nor P3074/P3726 (trivial unions)
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 37:15 note "in call to 'read_unconstructed()'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:3 note "construction of subobject of member 's' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 43:3 note "in call to 'start_lifetime<int[2]>(w.s.x)'"
//@ diag: 46:15 note "in call to 'inactive_container()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T> constexpr void start_lifetime(T &r) noexcept {
  __builtin_start_lifetime(__builtin_addressof(r));
}
} // namespace std

struct S {
  int v;
  constexpr S(int x) : v(x) {}
};
union U { S a[3]; char c; };

constexpr int read_unconstructed() {
  U u;
  std::start_lifetime(u.a);
  return u.a[1].v;
}
static_assert(read_unconstructed() == 0);

union W { struct { int x[2]; } s; int i; };
constexpr int inactive_container() {
  W w;
  w.i = 1;
  std::start_lifetime(w.s.x);
  return 0;
}
static_assert(inactive_container() == 0);
