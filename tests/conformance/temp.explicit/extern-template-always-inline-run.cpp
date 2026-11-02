// [temp.explicit]/12 (explicit instantiation declarations do not suppress
// the implicit instantiation of inline functions), [dcl.attr.grammar]:
// an `always_inline` inline member of a class template under `extern
// template` is odr-used, but no translation unit provides its definition
// (the reduction of libstdc++'s `extern template class allocator<char>`,
// whose `allocate`/`deallocate` libstdc++.so does not export). Like
// clang, kiln emits it `available_externally` and the O0 pipeline's
// always-inliner inlines every call, so the program links.
// `[[__gnu__::__always_inline__]]` is the reserved spelling of
// `[[gnu::always_inline]]`.
//@ expect: run
//@ strict-diags
//@ stdout: 3 abc 7 9
extern "C" int printf(const char *, ...);

template <class T> struct Alloc {
  __attribute__((__always_inline__)) constexpr T *allocate(unsigned long n) {
    return new T[n];
  }
  [[__gnu__::__always_inline__]] constexpr void deallocate(T *p) { delete[] p; }
  [[gnu::always_inline]] inline int seven() const;
  template <class U> [[gnu::always_inline]] U conv(U u) const { return u + 2; }
};
template <class T> int Alloc<T>::seven() const { return 7; }
extern template struct Alloc<char>;

int main() {
  Alloc<char> a;
  char *p = a.allocate(4);
  p[0] = 'a', p[1] = 'b', p[2] = 'c', p[3] = 0;
  printf("%d %s %d %d\n", 3, p, a.seven(), a.conv(7));
  a.deallocate(p);
}
