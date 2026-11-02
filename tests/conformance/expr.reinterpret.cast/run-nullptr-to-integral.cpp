// [expr.reinterpret.cast]/4: a value of type std::nullptr_t converts to an
// integral type; the conversion has the meaning of `(void*)0`'s. Also from a
// dependent operand that becomes std::nullptr_t (libc++ formatter.pointer
// test's `reinterpret_cast<std::uintptr_t>(arg)`). WP-296.
//@ expect: run
//@ stdout: 0 0 0 1
extern "C" int printf(const char*, ...);
using nullptr_t = decltype(nullptr);

template <class P> unsigned long as_int(P p) { return reinterpret_cast<unsigned long>(p); }

int main() {
  nullptr_t n = nullptr;
  unsigned long a = reinterpret_cast<unsigned long>(n);
  long long b = (long long)nullptr;
  int x = 0;
  printf("%lu %lld %lu %d\n", a, b, as_int(nullptr), as_int(&x) != 0);
}
