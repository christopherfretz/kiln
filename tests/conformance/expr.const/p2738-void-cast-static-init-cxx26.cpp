// [expr.static.cast]/14 (C++26, P2738R1), [basic.start.static]/2: a
// conversion from `void *` to a pointer to the type of the object it
// points to is a core constant expression, so the variable is
// constant-initialized before any dynamic initialization: `e` reads 4.
// p2738-void-cast-pre26-static-init.cpp is the pre-C++26 case (WP-407).
//@ std: c++26
//@ expect: run
//@ stdout: 4 4
extern "C" int printf(const char *, ...);
extern int g;
int e = g;
constexpr int n = 4;
constexpr const void *vp = &n;
int g = *(const int *)vp;
int main() { printf("%d %d\n", e, g); }
