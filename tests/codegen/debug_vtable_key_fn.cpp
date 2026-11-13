// Debug info (WP-1372): vtable homing by key function. Here's key function
// (key) is defined in this unit: a definition, without an
// identifier (its vtable is external, clang needsTypeIdentifier).
// Elsewhere's key function is not defined here: a declaration, as is
// NoKey's (no key function, no vtable emitted here, only used through a
// reference). HereDerived has a virtual base and its key function g is
// defined here: a definition with a DIFlagVirtual inheritance.
//@ args: -g
namespace n {
struct Here {
  virtual int key();
  virtual int f() { return 1; }
  int x = 0;
};
int Here::key() { return x; }
struct Elsewhere {
  virtual ~Elsewhere();
  virtual int f() { return 2; }
  int y = 0;
};
struct HereDerived : virtual Here {
  virtual int g();
  int z = 0;
};
int HereDerived::g() { return z; }
struct NoKey {
  virtual int f() { return 3; }
  int k = 0;
};
} // namespace n
int use(n::Here &h, n::Elsewhere &e, n::HereDerived &d, n::NoKey &k) {
  return h.f() + e.f() + d.g() + k.f() + k.k;
}
