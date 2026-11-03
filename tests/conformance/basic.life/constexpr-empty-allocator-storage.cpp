// [basic.life]/1, [allocator.members], [expr.const]/5.18: an object of an
// empty class in std::allocator storage (libc++'s vector<bitset<0>>) is
// within its lifetime from its placement new (std::construct_at) to its
// destruction. As a complete object or array element it owns its byte,
// so destroying it twice is not constant; an empty subobject sharing its
// byte (an empty base, [[no_unique_address]]) touches no other member's.
// spec/oven.md "Transient allocation".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 79:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:58 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 75:3 note "in call to 'destroy_at<Empty>(&{*new Empty[1]#0}[0])'"
//@ diag: 79:15 note "in call to 'destroy_twice()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T> struct allocator {
  constexpr T *allocate(size_t n) { return static_cast<T *>(operator new(n * sizeof(T))); }
  constexpr void deallocate(T *p, size_t) { operator delete(p); }
};
template <class T, class... U> constexpr T *construct_at(T *p, U... u) { return ::new ((void *)p) T(u...); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

struct Empty {};
struct EmptyBase {
  constexpr EmptyBase() noexcept {}
};
struct Derived : private EmptyBase {
  constexpr Derived() noexcept {}
};
struct Holder {
  [[no_unique_address]] Empty e;
  int v;
};

// An empty object in allocator storage is constructed, copied and
// destroyed like any other: each element owns its byte.
template <class T> constexpr bool round_trip() {
  std::allocator<T> a;
  T *p = a.allocate(2);
  T x{};
  std::construct_at(p, x);
  std::construct_at(p + 1);
  std::destroy_at(p);
  std::destroy_at(p + 1);
  std::construct_at(p);
  std::destroy_at(p);
  a.deallocate(p, 2);
  return true;
}
static_assert(round_trip<Empty>());
static_assert(round_trip<Derived>());

// A [[no_unique_address]] member shares its byte with `v`: constructing
// it anew leaves `v` alone.
constexpr int shared() {
  std::allocator<Holder> a;
  Holder *p = a.allocate(1);
  std::construct_at(p, Empty{}, 5);
  std::construct_at(&p->e);
  int v = p->v;
  std::destroy_at(p);
  a.deallocate(p, 1);
  return v;
}
static_assert(shared() == 5);

// Its lifetime is tracked: destroying it twice is not constant.
constexpr bool destroy_twice() {
  std::allocator<Empty> a;
  Empty *p = a.allocate(1);
  std::construct_at(p);
  std::destroy_at(p);
  std::destroy_at(p);
  a.deallocate(p, 1);
  return true;
}
static_assert(destroy_twice());
