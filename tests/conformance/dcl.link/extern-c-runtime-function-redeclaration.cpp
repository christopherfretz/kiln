// [dcl.link]/7: two declarations of an `extern "C"` function in different
// namespaces declare the same function. A user declaration of an Itanium
// runtime function codegen also calls (`__cxa_allocate_exception` for a
// `throw`) is that same symbol, not a renamed copy that fails to link
// (libc++ <future>/<valarray> tests, wait.pass.cpp). Matches clang 19.
//@ expect: run
//@ stdout: 7 1
extern "C" int printf(const char*, ...);
namespace __cxxabiv1 {
extern "C" void* __cxa_allocate_exception(unsigned long) noexcept;
extern "C" void __cxa_free_exception(void*) noexcept;
}
namespace a { extern "C" int twice(int); }
namespace b { extern "C" int twice(int x) { return 2 * x; } }
int f(int x) {
  if (x)
    throw x;
  return 0;
}
int main() {
  void* p = __cxxabiv1::__cxa_allocate_exception(4);
  __cxxabiv1::__cxa_free_exception(p);
  int r = 0;
  try {
    f(7);
  } catch (int v) {
    r = v;
  }
  printf("%d %d\n", r, a::twice(1) == b::twice(1));
}
