// [expr.unary.op]/4: `&C::m` names a member of a class that is still
// being defined (a static data member initializer of `C`, or of a class
// nested in it). The value needs no layout; using it after the class is
// complete reaches the member (WP-792, a JSON library's field metadata).
//@ std: c++20
// expected-no-diagnostics

struct F {
  char tag;
  int name;
  long other;
  constexpr int get() const { return name; }
  static constexpr auto v = &F::name;
  static constexpr auto w = &F::other;
  static constexpr auto g = &F::get;
  static constexpr bool same = v == &F::name;
  static constexpr bool differ = v != nullptr;
  struct G {
    using T = F;
    static constexpr auto v = &T::name;
    static constexpr auto both = &F::other;
  };
};

static_assert(F::same && F::differ);
static_assert(F::v == &F::name);
static_assert(F::G::v == F::v);
static_assert(F::G::both == F::w);

constexpr F f{'a', 7, 9};
static_assert(f.*F::v == 7);
static_assert(f.*F::w == 9);
static_assert(f.*F::G::v == 7);
static_assert((f.*F::g)() == 7);

struct D : F {
  int extra = 3;
};
constexpr D d{{'b', 11, 13}};
constexpr int D::*dv = F::v;
static_assert(d.*dv == 11);
static_assert(d.*F::w == 13);
