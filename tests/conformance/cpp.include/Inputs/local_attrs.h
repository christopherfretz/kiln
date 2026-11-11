#ifndef LOCAL_ATTRS_H
#define LOCAL_ATTRS_H
// The shape of libstdc++'s <bits/c++config.h> std::__terminate: a
// block-scope redeclaration with GNU attributes.
namespace la {
void stop(int) noexcept;
inline void call_stop(int c) noexcept {
  void stop(int) noexcept __attribute__((__noreturn__, __cold__));
  if (c != 0)
    stop(c);
}
inline int locals() {
  [[maybe_unused]] int a = 3;
  __attribute__((unused)) int b = 4;
  [[maybe_unused]] static const int c = 5;
  return a + b + c;
}
}
#endif
