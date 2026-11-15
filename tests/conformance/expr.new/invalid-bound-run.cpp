// [expr.new]/8: an invalid array bound evaluated at run time (negative before the conversion to
// std::size_t, a size past the implementation's limit, PTRDIFF_MAX bytes here, also through the
// array cookie, the constant inner dimension or a class's conversion function, or fewer elements
// than initializers) means the allocation function is not called: the new-expression throws
// std::bad_array_new_length (caught as std::bad_alloc too), or yields null for a non-throwing
// allocation function. clang passes SIZE_MAX to the allocation function instead; GCC throws even
// for the nothrow form. Found running GCC's C++ testsuite.
//@ expect: run
//@ args: -std=c++20
//@ skip(clang): clang 19/22 call the allocation function with SIZE_MAX ([expr.new]/8.6)
//@ skip(gcc): GCC 16 throws bad_array_new_length from the nothrow form ([expr.new]/8.6.1)
//@ stdout: ok
#include <cstdio>
#include <cstdlib>
#include <new>
int fails = 0;
void check(bool ok, int line) { if (!ok) { std::printf("line %d\n", line); ++fails; } }
#define CHECK(...) check((__VA_ARGS__), __LINE__)
struct S { int x; S() : x(1) {} ~S() {} };
int class_calls = 0;
struct K {
  int k;
  static void* operator new[](std::size_t n) { ++class_calls; return std::malloc(n); }
  static void operator delete[](void* p) { std::free(p); }
};
struct Bound { int v; operator int() const { return v; } };
template <class F> bool throws_bad_length(F f) {
  try { f(); } catch (const std::bad_array_new_length&) { return true; } catch (...) { return false; }
  return false;
}
int main(int argc, char**) {
  volatile int neg = -argc;            // -1
  volatile long long big = 1LL << 62;  // overflows n * sizeof(int)
  volatile std::size_t huge = ~std::size_t(0) / 8;  // fits n * 1, not n * 8 + cookie
  volatile int three = 3;
  CHECK(throws_bad_length([&] { return new int[neg]; }));
  CHECK(throws_bad_length([&] { return new char[neg]; }));
  CHECK(throws_bad_length([&] { return new int[big]; }));
  CHECK(throws_bad_length([&] { return new S[huge]; }));          // cookie
  CHECK(throws_bad_length([&] { return new char[huge * 5]; }));    // beyond PTRDIFF_MAX
  CHECK(throws_bad_length([&] { return new int[neg][3]; }));
  CHECK(throws_bad_length([&] { return new int[big][3]; }));
  CHECK(throws_bad_length([&] { return new int[Bound{neg}]; }));
  CHECK(throws_bad_length([&] { return new int[three]{1, 2, 3, 4}; }));
  CHECK(throws_bad_length([&] { return new K[neg]; }) && class_calls == 0);
  // The handler for std::bad_alloc catches it too.
  bool alloc = false;
  try { (void)new int[neg]; } catch (const std::bad_alloc&) { alloc = true; }
  CHECK(alloc);
  // A non-throwing allocation function: null, the function not called.
  CHECK(new (std::nothrow) int[neg] == nullptr);
  CHECK(new (std::nothrow) S[huge] == nullptr);
  CHECK(new (std::nothrow) int[three]{1, 2, 3, 4} == nullptr);
  // Valid bounds still allocate.
  int* p = new int[three]{1, 2, 3};
  S* q = new S[three];
  K* k = new K[three];
  CHECK(p[2] == 3 && q[2].x == 1 && class_calls == 1);
  delete[] p;
  delete[] q;
  delete[] k;
  if (!fails) std::puts("ok");
  return fails;
}
