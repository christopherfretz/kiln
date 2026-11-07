// [expr.prim.splice]/2.1.4 (P2996R13): only a potentially-evaluated splice
// of a local across a lambda is ill-formed. Unevaluated operands are fine
// (sizeof, decltype, and a typeid whose operand is itself unevaluated:
// `typeid(sizeof([: r :]))`; `sizeof(typeid([: r :]))` is unevaluated
// too). A constexpr local holding a reflection of a namespace-scope
// variable or of a type is not odr-used by the splice operand
// ([basic.def.odr]/5), so a captureless lambda may splice it, and a
// splice of a local in its own function is fine. GCC 16.1 compiles and
// runs the same file.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 10 1 2 4 5 1 1
#include <meta>
#include <cstdio>
#include <typeinfo>

int g = 7;

void f(int p) {
  int x = 2;
  constexpr auto rg = ^^g;
  constexpr auto rp = ^^p;
  constexpr auto rx = ^^x;
  constexpr auto rt = ^^int;
  auto a = [] {
    typename [: rt :] v = 3;
    return [: rg :] + v;
  };
  auto b = [&] { return sizeof([: rp :]); };
  auto c = [] { return decltype([: rx :]){5}; };
  auto d = [] { return &typeid(sizeof([: rx :])) == &typeid(decltype(sizeof 0)); };
  auto e = [] { return sizeof(typeid([: rx :])) == sizeof(std::type_info); };
  std::printf("%d %d %d %d %d %d %d\n", a(), [: rp :], [: rx :], (int)b(), c(),
              (int)d(), (int)e());
}

int main() { f(1); }
