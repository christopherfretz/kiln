// Interface unit of module `img` for the module.interface image tests
// (spec/modules.md §11.11 "Part 2a"): every declaration here is one the
// declaration image carries, so under `-fkiln-bmi-image` an importer
// builds it from the image instead of replaying this source.
export module img;
namespace geo {
export int add(int a, int b);
int add(int a, int b) { return a + b; }
export enum class Color : unsigned char { red, green = 5, blue };
export enum Sign { neg = -3, zero = 0, pos };
export enum Wide : unsigned long long { big = 0xffffffffffffull };
export typedef int Int;
export using IntPtr = const Int *;
export using Fn = int (*)(int, int);
export Fn pick();
Fn pick() { return &add; }
static int helper(int x) { return x + 1; }
export int bump(int x) noexcept { return helper(x); }
} // namespace geo
export {
  long twice(long x);
  using Quad = int[4];
  int first(const Quad &q);
  long sink(long &&v);
  int count(const char *fmt, ...);
}
long twice(long x) { return 2 * x; }
int first(const Quad &q) { return q[0]; }
long sink(long &&v) { return v + 1; }
