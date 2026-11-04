// [expr.prim.lambda.capture]/8, [dcl.type.decltype]/1: a visitor over a
// two-alternative node casts a captured reference to the type of its own
// parameter, `decltype(lhs)` with the reference removed; in each
// specialization `&rhs` names the closure's reference member, so the
// cast yields the enclosing function's referee (WP-581).
//@ expect: run
//@ std: c++20
//@ args: -O2
//@ stdout: 1 0 1 0 0
extern "C" int printf(const char*, ...);
template <class T> struct remove_ref { using type = T; };
template <class T> struct remove_ref<T&> { using type = T; };

struct Node { int kind; };
struct Int : Node { int v; };
struct Dbl : Node { double d; };
static bool operator==(const Int& a, const Int& b) { return a.v == b.v; }
static bool operator==(const Dbl& a, const Dbl& b) { return a.d == b.d; }

template <class F>
bool visit(const Node& n, F&& f) {
  switch (n.kind) {
  case 0: return static_cast<F&&>(f)(static_cast<const Int&>(n));
  default: return static_cast<F&&>(f)(static_cast<const Dbl&>(n));
  }
}

static bool equal(const Node* l, const Node* r) {
  if (l->kind != r->kind) return false;
  const Node& rhs = *r;
  return visit(*l, [&](const auto& lhs) {
    return lhs == *reinterpret_cast<typename remove_ref<decltype(lhs)>::type*>(&rhs);
  });
}

int main() {
  Int a{{0}, 3}, b{{0}, 3}, c{{0}, 4};
  Dbl x{{1}, 1.5}, y{{1}, 1.5}, z{{1}, 2.5};
  printf("%d %d %d %d %d\n", equal(&a, &b), equal(&a, &c), equal(&x, &y), equal(&x, &z),
         equal(&a, &x));
}
