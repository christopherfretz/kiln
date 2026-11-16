// [meta.member]/2, /4, [class.union.anon]: std::is_pointer_interconvertible_with_class
// and std::is_corresponding_member on a pointer to a member of an anonymous
// struct inside an anonymous union, when the pointer is a function
// parameter (not folded from `&C::m`) in constant evaluation and at run
// time. Both answers each. Found running GCC's C++ testsuite (WP-1497).
//@ skip(clang): clang 19/22 do not have the builtins libstdc++ gates on
//@ expect: run
//@ exit: 5
//@ strict-diags
//@ std: c++20
//@ args: -stdlib=libstdc++
#include <type_traits>

struct S { int x; union { struct { int a; int b; }; int c; }; };
struct U { union { struct { int a; int b; }; int c; }; };
struct T { int a; int b; };

constexpr bool pic(int S::*p) { return std::is_pointer_interconvertible_with_class(p); }
constexpr bool picU(int U::*p) { return std::is_pointer_interconvertible_with_class(p); }
constexpr bool cm(int U::*p, int T::*q) { return std::is_corresponding_member(p, q); }

static_assert(std::is_pointer_interconvertible_with_class(&U::a));
static_assert(!std::is_pointer_interconvertible_with_class(&S::a));
static_assert(pic(&S::x));
static_assert(!pic(&S::a));
static_assert(!pic(&S::b));
static_assert(!pic(&S::c));
static_assert(picU(&U::a));
static_assert(!picU(&U::b));
static_assert(picU(&U::c));
static_assert(!cm(&U::a, &T::a));  // the anonymous union is not an int

int main() {
  int U::*a = &U::a;
  int U::*b = &U::b;
  int S::*x = &S::x;
  int S::*sa = &S::a;
  return picU(a) + 2 * picU(b) + 4 * pic(x) + 8 * pic(sa) + 16 * cm(a, &T::a);
}
