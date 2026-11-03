// [basic.life]/8, [expr.const]/5.8, [specialized.construct]: a member of
// empty class type (one holding no data) destroyed by `std::destroy_at` and
// re-created by `std::construct_at` is within its lifetime again: a member
// call (its assignment) and a second destroy-and-construct are constant.
// libc++'s `__movable_box` assigns a non-assignable `single_view` element
// this way. spec/oven.md "Object lifetime".
//@ args: -stdlib=libc++
//@ expect: compile-pass
#include <memory>
struct E {
  E() = default;
  E(const E &) = default;
  constexpr E &operator=(const E &) { return *this; }
};
struct Holder {
  E v;
};
struct Box {
  [[no_unique_address]] Holder h;
  constexpr Box &operator=(const Box &o) {
    if (this != &o) {
      std::destroy_at(&h.v);
      std::construct_at(&h.v, o.h.v);
    }
    return *this;
  }
  Box() = default;
};
struct View {
  [[no_unique_address]] Box b;
};
constexpr bool twice() {
  const View a;
  View c;
  c.b = a.b;
  c.b = a.b;
  return true;
}
static_assert(twice());
constexpr bool assign_after() {
  Holder c;
  std::destroy_at(&c.v);
  std::construct_at(&c.v);
  c.v = E();
  return true;
}
static_assert(assign_after());
int main() {}
