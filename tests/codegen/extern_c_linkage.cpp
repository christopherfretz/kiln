// WP-492, [dcl.link]/4-5, [basic.link]/3-4: language linkage inside
// `extern "C"`. An entity with internal linkage has no language linkage
// and is mangled (`static`, a `const` variable in a braced
// specification: `_ZL2sx`, `_ZN1ML2msE`), and its functions keep
// `noundef` on the return; a function or variable of an unnamed
// namespace first declared in `extern "C"` keeps external linkage and
// its plain name; a block-scope `extern` declaration and a friend
// declared in a class take the linkage specification lexically
// enclosing their function or class (clang `isInExternCContext`); class
// members have C++ linkage.
extern "C" { extern int x; }
extern "C" { static int sx = 3; }
extern "C" { static int sf(int a) { return a + 1; } }
extern "C" { int y; }
extern "C" int z;
extern "C" { extern int w; int w = 5; }
extern "C" { extern int u; }
int u = 9;
namespace N { extern "C" { extern int v; } }
extern "C" { namespace M { static int ms = 2; static void mf() {} } }
extern "C" { static int sy; }
extern "C" { static void sg(); }
static void sg() {}
extern "C" { const int ci = 4; }
extern "C" const int ce = 6;
extern "C" { constexpr int cx = 7; }
extern "C" { extern const int eci; }
extern "C" { namespace { int an = 1; void af() {} } }
extern "C" { void outer() { extern int q; void lf(int); lf(q); } }
extern "C" { inline namespace I { extern int iv; } }
extern "C" { struct S { static int sm; static void smf(); }; }
int S::sm = 1;
void S::smf() {}
extern "C" { inline void il() { static int ls = 4; ls++; } }
extern "C" { struct T { void m() { void lm(); lm(); } }; }
extern "C" { struct FS { friend void fr(); }; }
void fr() {}
int use() {
  sg(); M::mf(); af(); S::smf(); il(); T().m();
  return x + sx + sf(1) + y + z + w + u + N::v + M::ms + sy + ci + ce + *&cx
       + eci + an + iv + S::sm;
}
