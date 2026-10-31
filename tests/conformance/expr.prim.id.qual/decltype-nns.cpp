// [expr.prim.id.qual]/1, [basic.lookup.qual.general]/1: a
// computed-type-specifier `decltype(e)` may begin a nested-name-specifier;
// a class or enumeration type names the scope looked into. The qualifier
// is accepted in every place a nested-name-specifier is (declarations,
// `typename`, expressions, template-ids, base-specifiers, member pointers,
// template arguments, casts, statements and conditions).
//@ expect: compile-ok
struct true_type {
  static constexpr bool value = true;
  using type = true_type;
};
struct S {
  using type = int;
  static constexpr int v = 3;
  enum E { A, B };
  template <class T> struct M { using type = T; };
  int m;
  static int f(int x) { return x; }
};
S s;
true_type make_true();
enum class Color { red, green };
Color c;

// Expressions and constant expressions (libc++'s `__has_iterator_typedefs`).
static const bool b1 = decltype(make_true())::value;
static_assert(decltype(make_true())::value);
static_assert(decltype(s)::v == 3);
static_assert(decltype(s)::B == 1);
static_assert(decltype(c)::green == Color::green);

// Declarations, with and without `typename`; template-ids in the qualifier.
typename decltype(s)::type x1 = 1;
decltype(s)::type y1 = 2;
decltype(s)::template M<int>::type z1 = 3;
decltype(s)::M<long>::type w1 = 4L;
using U = decltype(s)::type;
U u1 = 5;

// Member pointers.
int decltype(s)::*pm = &S::m;

// Base-specifiers.
struct D1 : decltype(s) {};
struct D2 : decltype(s)::M<int> {};
struct D3 : decltype(s)::template M<char> {};

namespace N { S get(); }
template <class T> struct W { T t; };

int g() {
  decltype(s)::type q = decltype(s)::v;
  decltype(N::get())::type r = 0;
  int a = decltype(s)::f(2) + decltype(s)::v;
  if (decltype(s)::v == 3) {}
  if (decltype(s)::type z = 1) { (void)z; }
  for (decltype(s)::type i = 0; i < 2; ++i) {}
  W<decltype(s)::type> w{1};
  W<decltype(s)::M<char>::type> w2{'a'};
  int c1 = (decltype(s)::type)1.5;
  decltype(s)::type d(3);
  decltype(s)::type e{4};
  int f = decltype(s)::type(5);
  int h = decltype(s)::type{6};
  auto k = sizeof(decltype(s)::type *);
  int decltype(s)::*lpm = &S::m;
  (void)lpm;
  (void)w2;
  return q + r + a + c1 + d + e + f + h + (int)k + w.t;
}
