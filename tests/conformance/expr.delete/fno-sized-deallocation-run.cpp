// [expr.delete]/10: under -fno-sized-deallocation a delete-expression of
// a complete type calls the unsized replaceable `operator delete`, even
// with a sized one defined (clang LangOptions::SizedDeallocation). WP-808.
//@ expect: run
//@ args: -fno-sized-deallocation
#include <cstdlib>
#include <new>
static int unsized, sized;
void* operator new(std::size_t n) { return std::malloc(n ? n : 1); }
void operator delete(void* p) noexcept { ++unsized; std::free(p); }
void operator delete(void* p, std::size_t) noexcept { ++sized; std::free(p); }
struct A { int x[4]; };
struct V { virtual ~V() {} int y; };
struct W : V { int z; };
int main() {
  delete new A;
  delete new int(3);
  V* v = new W;
  delete v;
#ifdef __cpp_sized_deallocation
  return 3;
#endif
  return unsized == 3 && sized == 0 ? 0 : 1;
}
