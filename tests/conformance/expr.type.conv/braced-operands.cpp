// [expr.type.conv]/1, /2: the parenthesized expression-list of a
// functional cast is an expression-list of initializer-clauses, so an
// operand may be a braced-init-list, which list-initializes its
// constructor parameter or aggregate element ([dcl.init.general]/16.6;
// libc++ `pair<T, U>({}, {})`), also in a template, where the cast is
// rebuilt at instantiation. Matches clang 19.
//@ expect: run
struct A {
  int x, y;
  constexpr A(int a, int b) : x(a), y(b) {}
};
struct V {
  int a;
  constexpr V(A p) : a(p.x + p.y) {}
};
template <class T, class U> struct Pair {
  T first;
  U second;
  constexpr Pair(const T& t, const U& u) : first(t), second(u) {}
};
struct Agg {
  int a;
  int b;
};
static_assert(A(1, {}).y == 0);
static_assert(V({2, 3}).a == 5);
static_assert(Pair<int, A>({}, {4, 5}).second.y == 5);
static_assert(Agg({6}, {}).a == 6);
template <class T> constexpr T make() { return T({}, {7}); }
static_assert(make<Pair<int, int>>().second == 7);
template <class T> constexpr int lit(T v) { return Pair<T, T>({v}, {v}).first; }
static_assert(lit(8) == 8);
int main() { return make<Pair<int, int>>().second == 7 ? 0 : 1; }
