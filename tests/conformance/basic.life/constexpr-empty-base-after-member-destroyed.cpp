// [basic.life]/1, /8, [class.union.general]/6, [expr.const]/10.8: an empty
// base class subobject has no storage of its own, so destroying the
// member that shares its address (a union member, or any member) does not
// end its lifetime; a member call on it stays valid, and a new object
// constructed in the member's storage (std::construct_at) is used as
// before. The shape of libstdc++ 14's std::optional before C++26
// (`emplace` after `emplace` or `reset`: `this->_M_construct` on the
// empty `_Optional_base_impl` after the payload's destructor ran); kiln
// read the dead byte under the empty base as the base's own and said
// "member call on object outside its lifetime". The C++26 sibling is
// constexpr-empty-base-after-member-destroyed-cxx26.cpp; the rejected
// side constexpr-empty-base-after-member-destroyed-errors.cpp.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
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
struct Empty {};

// The shape of libstdc++'s optional: an empty base whose member functions
// reach the derived object's union member, destroyed and constructed
// again between two calls on the empty base.
template <class T> union Storage {
  constexpr Storage() : e() {}
  constexpr ~Storage() {}
  Empty e;
  T value;
};
template <class D, class T> struct Impl {
  constexpr D &self() { return static_cast<D &>(*this); }
  constexpr void reset() {
    if (self().on) {
      self().on = false;
      self().s.value.~T();
    }
  }
  template <class... A> constexpr void construct(A... a) {
    std::construct_at(&self().s.value, a...);
    self().on = true;
  }
};
template <class T> struct Opt : Impl<Opt<T>, T> {
  Storage<T> s;
  bool on = false;
  template <class... A> constexpr T &emplace(A... a) {
    this->reset();
    this->construct(a...);
    return s.value;
  }
};

constexpr int after_reset() {
  Opt<S> o;
  o.emplace(3);
  o.reset();
  return o.emplace(4).v;
}
static_assert(after_reset() == 4);
constexpr int twice() {
  Opt<S> o;
  o.emplace(3);
  return o.emplace(4).v;
}
static_assert(twice() == 4);
constexpr int scalar() {
  Opt<int> o;
  o.emplace(3);
  o.reset();
  return o.emplace(4);
}
static_assert(scalar() == 4);

// Without a union: destroying a member leaves the object and its empty
// base within their lifetime ([basic.life]/1).
struct Tag {
  constexpr int id() const { return 5; }
};
struct D : Tag {
  S s;
  int k = 1;
};
constexpr int member_destroyed() {
  D d{{}, S(2)};
  d.s.~S();
  int r = d.id();
  std::construct_at(&d.s, 6);
  return r + d.s.v;
}
static_assert(member_destroyed() == 11);
