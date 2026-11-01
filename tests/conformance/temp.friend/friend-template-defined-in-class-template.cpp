// [temp.friend]/1, [temp.inst]: a friend function template defined inside
// a class template is instantiated per enclosing specialization, and each
// specialization of the friend gets its definition from the pattern
// (WP-141: kiln used to panic, "decl has no Function payload").
//@ expect: run
template <class T> struct A {
  template <class U> friend constexpr int g(A, U u) { return sizeof(T) + u; }
  template <class U> friend bool h(A, U) { return true; }
};
static_assert(g(A<int>{}, 1) == 5);
static_assert(g(A<char>{}, 1) == 2);
int main() {
  A<int> a;
  bool b = h(a, 1);
  return g(A<int>{}, 2) - 6 + (b ? 0 : 1);
}
