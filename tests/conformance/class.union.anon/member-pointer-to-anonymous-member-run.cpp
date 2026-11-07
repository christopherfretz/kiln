// [class.union.anon]/1, [expr.unary.op]/4, [expr.mptr.oper]/6: a member
// of an anonymous union or struct is a member of the enclosing class, so
// `&C::m` forms a pointer to it whose offset is the member's offset in C
// (through the anonymous member). It is a constant expression
// ([expr.const]) usable as a template's static constexpr initializer, and
// `obj.*mp` reads and writes the member, at run time and in constant
// evaluation (of the active member: `.*` does not change a union's
// active member, [class.union.general]/6 names only `u.m = x`).
//@ expect: run
//@ strict-diags
struct E {
  char tag;
  union {
    int i;
    long l;
    float f;
  };
  struct {
    short a;
    double d;
  };
  int j;
};

constexpr int E::*pi = &E::i;
constexpr long E::*pl = &E::l;
constexpr double E::*pd = &E::d;
constexpr int E::*pj = &E::j;
static_assert(pi != nullptr && pi == &E::i, "");
static_assert(pi != pj, "");

// The shape of a type-indexed table of member pointers: a static
// constexpr variable template picks one by its type.
template <class T> struct pick;
template <> struct pick<int> { static constexpr int E::*value = &E::i; };
template <> struct pick<long> { static constexpr long E::*value = &E::l; };
template <class T> struct Tab {
  template <class U> static constexpr U E::*member = pick<U>::value;
};

constexpr int via_union() {
  E e{};       // i is the active member
  e.*pi = 41;
  return e.*pi + 1;
}
static_assert(via_union() == 42, "");

constexpr double via_struct() {
  E e{};
  e.*pd = 2.5;
  return e.*pd;
}
static_assert(via_struct() == 2.5, "");

struct D : E { int k; };
constexpr int E::*pb = pi;
constexpr int D::*pdi = pb;  // base-to-derived conversion keeps the member

int read(E &e, int E::*p) { return e.*p; }

int main() {
  E e{};
  e.i = 5;
  if (read(e, pi) != 5) return 1;
  e.*Tab<void>::member<long> = 9;
  if (e.l != 9) return 2;
  e.d = 1.5;
  if (e.*pd != 1.5) return 3;
  D d{};
  d.i = 6;
  if (d.*pdi != 6) return 4;
  int E::*rt = pj;
  e.j = 11;
  if (e.*rt != 11) return 5;
  if (reinterpret_cast<char *>(&(e.*pd)) != reinterpret_cast<char *>(&e.d)) return 6;
  return 0;
}
