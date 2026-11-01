// [dcl.attr.noreturn]: a noreturn function's declaration and definition
// carry `noreturn`; its calls are marked `noreturn` (not for a
// [[noreturn]] virtual function, whose overriders may return, unless the
// GNU spelling is used) and followed by `unreachable` (clang
// ConstructAttributeList / EmitCall).
[[noreturn]] void die();
[[noreturn]] void die2() { throw 1; }
struct S { [[noreturn]] void m(); [[noreturn]] virtual void v(); ~S(); [[noreturn]] static void st(); };
__attribute__((noreturn)) void gdie();
int f(int x) { if (x) die(); return x; }
int g(int x) { S s; if (x) die(); if (x > 1) s.m(); if (x > 2) s.v(); if (x > 3) s.st(); return x; }
int h(int x) { if (x > 3) gdie(); return 0; }
void (*fp)() = die;
int k(int x) { if (x) fp(); return x; }
struct G { __attribute__((noreturn)) virtual void gv(); [[noreturn]] virtual void v(); };
struct F final : G { [[noreturn]] void v() override; };
int g2(G &s, F &f, int x) { if (x) s.gv(); if (x > 1) s.G::v(); if (x > 2) f.v(); return x; }
[[noreturn]] void die3();
void die3() { die(); }
void q() { die3(); }
