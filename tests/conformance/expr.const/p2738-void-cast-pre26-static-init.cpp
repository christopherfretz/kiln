// [expr.const]/5.15 (C++11 to C++23), [basic.start.static]/2: before
// C++26 a conversion from `void *` to a pointer to object type is not a
// core constant expression, so a variable initialized through one is not
// constant-initialized and is initialized dynamically, in order: `e`
// reads `g` before its initializer runs (zero). Also when the
// initializer is first evaluated by code generation's folds, which share
// the translation unit's bytecode and must use its language mode
// (WP-407). p2738-void-cast-static-init-cxx26.cpp is the C++26 case.
//@ std: c++23
//@ expect: run
//@ stdout: 0 4 7
extern "C" int printf(const char *, ...);
extern int g, h;
int e = g, f = h;
constexpr int n = 4;
constexpr const void *vp = &n;
int g = *(const int *)vp;
int h = n + 3;
int main() { printf("%d %d %d\n", e, g, f); }
