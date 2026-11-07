// [class.union.general]/6, [class.dtor]/18, [basic.life]/1, [expr.const]/5.8: an explicit destructor
// call on a union's active member ends that member's lifetime, not the union's: the union may then
// be destroyed and constructed anew (libc++'s `expected` re-initialization: `destroy_at` of the
// member, then of the union, then `construct_at` of the union; WP-334). The member stays the active
// one, so reading it is a read outside its lifetime and reading another member is a read of an
// inactive member; destroying the member twice, or the union twice, is not constant. Pinned to C++23
// (P3726R2 changes union member destruction in C++26: basic.life/union-member-destroyed-cxx26.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:80 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 59:15 note "in call to 'read_member()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:79 note "read of member 'i' of union with active member 't' is not allowed in a constant expression"
//@ diag: 61:15 note "in call to 'read_other()'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:58 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 63:45 note "in call to 'destroy_at<T>(&r.u.t)'"
//@ diag: 65:15 note "in call to 'member_twice()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:58 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 67:67 note "in call to 'destroy_at<V>(&q.v)'"
//@ diag: 69:15 note "in call to 'union_twice()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A... a) { return ::new ((void *)p) T(a...); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

struct T { int v; constexpr T(int x) : v(x) {} constexpr ~T() {} };
union U {
  constexpr U(int x) : t(x) {}
  constexpr U(bool) : i(3) {}
  constexpr ~U() {}
  T t;
  int i;
};
struct R { U u; bool has; };
union V { int i; float f; };
struct Q { V v; bool has; };

constexpr bool reinit() {
  R r{U(5), true};
  std::destroy_at(&r.u.t);
  std::destroy_at(&r.u);
  std::construct_at(&r.u, false);
  r.has = false;
  return r.u.i == 3;
}
static_assert(reinit());
constexpr bool member_again() { R r{U(5), true}; std::destroy_at(&r.u.t); std::construct_at(&r.u.t, 7); return r.u.t.v == 7; }
static_assert(member_again());
constexpr bool assign_other() { R r{U(5), true}; std::destroy_at(&r.u.t); r.u.i = 4; return r.u.i == 4; }
static_assert(assign_other());

constexpr int read_member() { R r{U(5), true}; std::destroy_at(&r.u.t); return r.u.t.v; }
static_assert(read_member() == 5);
constexpr int read_other() { R r{U(5), true}; std::destroy_at(&r.u.t); return r.u.i; }
static_assert(read_other() == 5);
constexpr int member_twice() {
  R r{U(5), true}; std::destroy_at(&r.u.t); std::destroy_at(&r.u.t); return 1;
}
static_assert(member_twice());
constexpr int union_twice() {
  Q q{{1}, true}; std::destroy_at(&q.v.i); std::destroy_at(&q.v); std::destroy_at(&q.v); return 1;
}
static_assert(union_twice());
