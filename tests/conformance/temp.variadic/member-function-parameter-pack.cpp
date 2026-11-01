// [temp.variadic]/5, [temp.inst]/3: a member function of a class
// template whose parameter pack is expanded by the class's template
// arguments: each element is a parameter with the pack's name, not a
// redefinition ([dcl.fct]/12 concerns distinct declarators).
//@ expect: run
template <class... T> struct Tup {
  int n;
  constexpr Tup(T... t) : n((0 + ... + t)) {}
  constexpr int f(T... t) const { return n + (0 + ... + t); }
  static constexpr int count(T... t) { return sizeof...(t); }
};
static_assert(Tup<int, int, int>(1, 2, 3).n == 6);
static_assert(Tup<int, int>(1, 2).f(3, 4) == 10);
static_assert(Tup<int, char>::count(1, 'a') == 2);
static_assert(Tup<>().n == 0);
int main() { return Tup<int, int>(1, 2).f(3, -6); }
