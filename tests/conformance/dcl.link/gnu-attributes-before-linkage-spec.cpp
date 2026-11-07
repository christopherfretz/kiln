// [dcl.link]/2, GNU attribute syntax: an `__attribute__((...))` list
// written before a linkage-specification is part of the declaration
// specifiers of the declaration the braceless `extern "C"` contains, so
// it applies to that declaration; before a braced
// linkage-specification it is accepted and ignored (clang
// `ParseLinkage` with the outer DeclSpec; spec/parse.md "Attributes").
//@ expect: compile-ok
//@ strict-diags
//@ diag: 14:9 warning "'d' is deprecated"
//@ diag: 13:16 note "'d' has been explicitly marked deprecated here"
__attribute__((section("s"))) extern "C" int f(int);
__attribute__((visibility("hidden"))) extern "C" int v(int x) { return x; }
__attribute__((deprecated)) extern "C" int d();
int z = d();
__attribute__((deprecated)) extern "C" { int d2(); }
int z2 = d2();
__attribute__((unused)) extern "C++" int w(int);
__attribute__((section("t"))) __attribute__((weak)) extern "C" int g;
