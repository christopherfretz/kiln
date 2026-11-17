// the enclosing function is inlined everywhere; the closure's operator() is emitted
inline auto f() {
  return []() __attribute__((noinline)) { return VAL; };
}
using Fn = int (*)();
