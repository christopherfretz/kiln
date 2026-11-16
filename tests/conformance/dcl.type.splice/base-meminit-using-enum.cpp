// [dcl.type.splice], [class.derived.general]/1, [class.base.init]/1,
// [enum.udecl]/1 (P2996): a splice-type-specifier `[typename] [: r :]`
// names a base class in a base-specifier and in a mem-initializer
// (an inline constructor's included), and the enumeration of a
// using-enum-declarator; a splice followed by `::` still qualifies a
// class-name. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oracle: gcc
struct A { int a = 1; constexpr A() = default; constexpr A(int x) : a(x) {} };
namespace N { struct B { int b = 2; }; }
struct C : typename [: ^^A :] {};
struct D : public [: ^^A :] { constexpr D() : [: ^^A :](5) {} };
struct E : [: ^^N :]::B {};
struct F : virtual typename [: ^^A :] {
  constexpr F() : typename [: ^^A :](7) {}
};
template <auto R> struct G : [: R :] { constexpr G() : [: R :](9) {} };
static_assert(C{}.a == 1 && D{}.a == 5 && E{}.b == 2 && G<^^A>{}.a == 9);
int f() { return F{}.a; }

enum class Color { red, green };
constexpr int g() {
  using enum [: ^^Color :];
  return int(green);
}
static_assert(g() == 1);
