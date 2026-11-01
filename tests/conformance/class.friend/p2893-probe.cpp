// [class.friend]/3 (C++26, P2893R3): a friend declaration may name a pack
// expansion and a list of types. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(kiln): variadic friend declarations not parsed
//@ xfail(clang): clang 19 lacks P2893 (implemented in clang 20; clang 22 accepts)
template <class... Ts> class C {
  friend Ts...;
  int secret = 42;
};
struct A { static int get(const C<A, int>& c) { return c.secret; } };
class D { friend A, struct B; int z; };
int use() { return A::get(C<A, int>{}); }
