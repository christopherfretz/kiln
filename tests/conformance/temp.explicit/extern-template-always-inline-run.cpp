// [temp.explicit]/12 with GNU `always_inline` (WP-384): an inline member
// of a class under an explicit instantiation declaration whose library
// exports no symbol for it (libstdc++'s `[[__gnu__::__always_inline__]]
// std::allocator<char>::deallocate`, `extern template class
// allocator<char>`) is defined in this TU, as clang's -O0 always-inliner
// makes it; `__gnu__::` is the `gnu::` scope (clang
// `normalizeAttrScopeName`). No explicit instantiation definition exists,
// so the program links only if both members are emitted here.
//@ expect: run
//@ stdout: 3
extern "C" int printf(const char*, ...);
template <class T> struct A {
  [[__gnu__::__always_inline__]] constexpr T twice(T v) { return v + v; }
  __attribute__((__always_inline__)) A() {}
};
extern template struct A<int>;
int main() {
  A<int> a;
  printf("%d\n", a.twice(1) + 1);
}
