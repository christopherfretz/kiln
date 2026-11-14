// The explicit instantiation declarations of
// definition-after-pch-declaration-run.cpp, read from a PCH.
extern "C" int printf(const char *, ...);
namespace n {
template <class T> struct A {
  virtual int v() { return 11; }
  int f() { return 2; }
  int which() { return 0; }
};
template <> inline int A<long>::which() { printf("spec "); return 1; }
extern template struct A<long>;
} // namespace n
