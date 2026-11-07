// [dcl.link]/2, GNU attribute syntax: `__attribute__((...))` before a
// braceless `extern "C"` applies to the contained declaration (section,
// visibility, used); before a braced one it is ignored (clang
// `ParseLinkage` with the outer DeclSpec; spec/parse.md "Attributes").
__attribute__((section("s"))) extern "C" int f(int);
int f(int x) { return x; }
__attribute__((section("t"))) extern "C" int g;
int g = 1;
__attribute__((section("u"))) extern "C" { int h(int x) { return x; } }
__attribute__((noinline)) extern "C" int k(int x) { return x; }
__attribute__((visibility("hidden"))) extern "C" int vv(int x) { return x; }
int use() { return f(1) + g + h(2) + k(3) + vv(4); }
