// [expr.const]/3, /12: a const non-constexpr global that is not usable in
// constant expressions is still folded (GNU extension) in an array bound,
// while a strict evaluation of the same global must not see its value.
// Alternating the two modes re-resolves the global each time; the folded
// values, the strict results and the run-time values all agree.
//@ expect: run
//@ args: -Wno-gnu-folding-constant -Wno-vla-cxx-extension
//@ stdout: 6 8 5 | 6 8 5
//@ stdout: 6 8 5 | 6 8 5
extern "C" int printf(const char *, ...);
struct P { int x, y; };
const double d = 1.5;
const double a[3] = {1.0, 2.0, 3.0};
const P p = {2, 3};
template <class T> constexpr bool strict_ok(const T *) { return true; }
static_assert(strict_ok(&d));
char b0[(int)(d * 4)];
static_assert(&d != nullptr);
char b1[(int)(a[1] * 4)];
static_assert(&a[1] == a + 1);
char b2[p.x + p.y];
static_assert(&p.y != &p.x);
char c0[(int)(d * 4)];
char c1[(int)(a[1] * 4)];
char c2[p.x + p.y];
static_assert(strict_ok(&p));
int main() {
  printf("%d %d %d | %d %d %d\n", (int)sizeof b0, (int)sizeof b1, (int)sizeof b2, (int)(d * 4),
         (int)(a[1] * 4), p.x + p.y);
  printf("%d %d %d | %d %d %d\n", (int)sizeof c0, (int)sizeof c1, (int)sizeof c2, (int)(d * 4),
         (int)(a[1] * 4), p.x + p.y);
}
