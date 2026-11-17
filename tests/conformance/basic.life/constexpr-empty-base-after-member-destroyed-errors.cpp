// [basic.life]/1, /7, [expr.const]/10.8: the accepted side is
// constexpr-empty-base-after-member-destroyed.cpp. An empty base class
// subobject ends with its derived object; an empty object of its own (a
// variable, an array element) ends with its explicit destruction. A member
// call then is not a constant expression.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:13 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 49:15 note "in call to 'whole_destroyed()'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:12 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 56:15 note "in call to 'own_destroyed()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:17 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 66:15 note "in call to 'element_destroyed()'"
namespace std {
using size_t = decltype(sizeof 0);
}
constexpr void *operator new(std::size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A &&...a) {
  return ::new ((void *)p) T(static_cast<A &&>(a)...);
}
} // namespace std

struct S {
  int v;
  constexpr S(int x) : v(x) {}
  constexpr ~S() {}
};
struct Tag {
  constexpr int id() const { return 5; }
};
struct D : Tag {
  S s;
  int k = 1;
  constexpr ~D() {}
};
// The derived object itself destroyed: its empty base is gone too.
constexpr int whole_destroyed() {
  D d{{}, S(2)};
  d.~D();
  int r = d.id();
  std::construct_at(&d, D{{}, S(1)});
  return r;
}
static_assert(whole_destroyed() == 5);
// An empty object of its own (not a base) destroyed.
constexpr int own_destroyed() {
  Tag t;
  t.~Tag();
  return t.id();
}
static_assert(own_destroyed() == 5);
struct H {
  Tag t[2];
  int k;
};
constexpr int element_destroyed() {
  H h{};
  h.t[1].~Tag();
  return h.t[1].id();
}
static_assert(element_destroyed() == 5);
