// [basic.def.odr]/4-5: reading a `static const` integral member that is
// usable in constant expressions is not an odr-use, so it needs no
// out-of-line definition -- also inside a try-block, a handler and a
// function-try-block, and through a default argument (libc++
// `memory_resource::allocate(size_t, size_t = __max_align)` called in a
// try-block, null_memory_resource.pass.cpp). The program links only if the
// reads are folded. Matches clang 19.
//@ expect: run
//@ stdout: 16 17 18 19 20
extern "C" int printf(const char*, ...);
struct R {
  static const unsigned long k = 16;
  static const int j = 17;
  unsigned long get(unsigned long a = k) { return a; }
};
struct E {};

int f() try {
  return R::j + 2;
} catch (...) {
  return 0;
}

int main() {
  R r;
  unsigned long a = 0, b = 0, c = 0;
  try {
    a = r.get();
    b = R::j;
    throw E{};
  } catch (const E&) {
    c = R::k + 2;
  }
  printf("%lu %lu %lu %d %d\n", a, b, c, f(), R::j + 3);
}
