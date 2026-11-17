// [meta.reflection.queries] (P2996R13): type_of needs an entity with a
// type ([basic.pre] has-type), which a structured binding is not, so
// type_of throws for one (and for each binding of a tuple-like
// decomposition); an enumerator reflected inside its enum-specifier has
// the type of its initializing value ([dcl.enum]/5: `int`, `long`, the
// underlying type of a fixed one), the enumeration after the closing
// brace. is_final sees `final` on a class while it is being defined.
// Found running GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <meta>
#include <tuple>
using namespace std::meta;
consteval bool has_type(info r) {
  try {
    (void)type_of(r);
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
struct P { int a; long b; };
constexpr P p{1, 2};
constexpr auto [pa, pb] = p;
static_assert(!has_type(^^pa) && !has_type(^^pb));
static_assert(has_type(^^p) && type_of(^^p) == ^^const P);
void f() {
  auto [x, y] = std::tuple<int, char>(1, 'c');
  static_assert(!has_type(^^x) && !has_type(^^y));
}

consteval int same(info x, info y) {
  if (x == y)
    return 0;
  throw 1;
}
enum E {
  A,
  B = 3L,
  C,
  D = 5ULL,
  Z = same(type_of(^^A), ^^int) + same(type_of(^^B), ^^long) +
      same(type_of(^^C), ^^long) + same(type_of(^^D), ^^unsigned long long)
};
enum F : long { FA, FB = 3, FZ = same(type_of(^^FA), ^^long) + same(type_of(^^FB), ^^long) };
enum class G { GA, GZ = same(type_of(^^GA), ^^int) };
static_assert(type_of(^^A) == (^^E) && type_of(^^FB) == (^^F) && type_of(^^G::GA) == ^^G);

struct K final {
  static_assert(is_final(^^K));
};
struct NK {
  static_assert(!is_final(^^NK));
};
template <class T> struct TK final {
  static_assert(is_final(^^TK));
};
template struct TK<int>;
static_assert(is_final(^^K) && !is_final(^^NK) && is_final(^^TK<int>));
