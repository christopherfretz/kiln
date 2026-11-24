// [class.temporary]/6, [temp.inst]: a reference bound through 300 nested
// glvalue conditional expressions (both arms xvalues) extends the
// temporary at the bottom of the tree, and an instantiated variable
// template specialization extends a copy of its own: each specialization
// constructs and destroys its own object. Lifetime extension stopped
// following the binding below 256 levels (the temporary was destroyed at
// once and the reference dangled), and an instantiation copied the
// pattern's temporaries only down to 256 levels, so the two
// specializations shared one (WP-3292).
//@ expect: run
//@ strict-diags
//@ stdout: +7 +7 +9 w9 | 1 7 7
//@ stdout: -9
//@ stdout: -7
//@ stdout: -7
extern "C" int printf(const char *, ...);
struct S {
  int v;
  constexpr S(int x) : v(x) {}
  S(int x, bool) : v(x) { printf("+%d ", v); }
  ~S() {
    if (v)
      printf("-%d\n", v);
  }
};
bool k;
const S g0(0);
#define X(e) static_cast<const S &&>(e)
#define C1(x) k ? X(g0) : x
#define C2(x) C1(C1(x))
#define C4(x) C2(C2(x))
#define C8(x) C4(C4(x))
#define C16(x) C8(C8(x))
#define C32(x) C16(C16(x))
#define C64(x) C32(C32(x))
#define C128(x) C64(C64(x))
#define C256(x) C128(C128(x))
#define C300(x) C256(C32(C8(C4(x))))
template <class T> const S &v = C300(X(S(7, true)));
const S &w = C300(X(S(9, true)));
int main() {
  const S *a = &v<int>;
  const S *b = &v<long>;
  printf("w%d | %d %d %d\n", w.v, a != b, a->v, b->v);
}
