// Pointer and reference non-type template arguments (WP-933): a whole
// object whose type differs from the parameter's pointee (`int` for
// `const int*`, `const volatile int&`) is mangled through a conversion to
// the parameter type, `XcvPKiadL_Z1gEE` / `XcvRKiL_Z1gEE`; a subobject or
// array element uses the `so` form; a reference or pointer whose type
// already matches stays `L_Z...E` / `XadL_Z...EE`. Internal-linkage
// objects keep their `L` (`_ZL1s`).
struct B { int x; };
struct D : B { int y; };
D d;
int arr[3];
int g;
const int cg = 1;
static int s = 2;
namespace { int an = 5; }
void fn() noexcept {}
constexpr const int* const cgp = &cg;
template <const int* P> int p1() { return 0; }
template <const int& R> int r1() { return 0; }
template <int& R> int r0() { return 0; }
template <int* P> int p0() { return 0; }
template <const volatile int* P> int p2() { return 0; }
template <const int* const* P> int pp() { return 0; }
template <void (*F)() noexcept> int pf() { return 0; }
int use() {
  return p1<&arr[1]>() + p1<arr>() + p1<&d.y>() + p1<&d.x>() + p1<&g>() + p1<&cg>() +
         p1<&s>() + p1<&an>() + r1<g>() + r1<cg>() + r1<d.y>() + r1<arr[2]>() + r0<g>() +
         p0<&g>() + p2<&g>() + p2<&cg>() + pp<&cgp>() + pf<fn>();
}
