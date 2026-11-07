// P3293R3 ([expr.ref]): `e.[: b :]`, with `b` a reflection of a direct
// base class relationship (from bases_of), designates the base class
// subobject, in constant evaluation and at run time. GCC 16.1 agrees.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
struct B { int b = 7; };
struct D : B { int d = 1; };
consteval std::meta::info base0() {
  return std::meta::bases_of(^^D, std::meta::access_context::unchecked())[0];
}
constexpr D dd{};
static_assert(dd.[: base0() :].b == 7);
int main() {
  D x;
  B& r = x.[: base0() :];
  r.b = 3;
  return x.b == 3 ? 0 : 1;
}
