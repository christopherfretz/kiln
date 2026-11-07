// [dcl.spec.auto.general]/9, [stmt.if]/2: a return statement in a
// discarded statement of a non-templated function takes no part in
// return type deduction, and (as in clang) its operand is not converted
// to the deduced type (a scanning library's `if constexpr` chain picking
// a tag type for the platform's FILE; WP-792).
//@ expect: run
//@ stdout: 1 2 3 4
extern "C" int printf(const char *, ...);

template <class T> struct tag { static constexpr int id = sizeof(T); };
struct S {};
constexpr bool yes = true;

constexpr auto pick() {
  if constexpr (yes) return tag<char>{};
  else if constexpr (!yes) return tag<long>{};
  else return tag<short>{};
}
static_assert(__is_same(decltype(pick()), tag<char>));
static_assert(decltype(pick())::id == 1);

auto later() {
  if constexpr (sizeof(int) == 1) return S{};
  return 2;
}
static_assert(__is_same(decltype(later()), int));

constexpr auto first() {
  if constexpr (!yes) return S{};
  else return 3L;
  if constexpr (!yes) return "no";
}
static_assert(__is_same(decltype(first()), long) && first() == 3);

auto none() {
  if constexpr (!yes) return 5;
}
static_assert(__is_same(decltype(none()), void));

int main() {
  none();
  printf("%d %d %ld %d\n", decltype(pick())::id, later(), first(),
         [] { if constexpr (yes) return 4; else return S{}; }());
  return 0;
}
