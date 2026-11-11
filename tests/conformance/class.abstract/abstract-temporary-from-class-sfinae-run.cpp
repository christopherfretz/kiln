// [temp.deduct.general]/8, [class.abstract]/3: an expression in a trailing
// return type that would bind a reference to an abstract class through a
// converting constructor (creating a temporary of it) is a substitution
// failure. Reduced from GoogleTest's printer probe `*os << value` for a class
// with a converting constructor of an abstract class that has an
// `operator<<`; kiln chose the stream printer and called the constructor.
//@ expect: run
//@ stdout: fallback call-fallback
#include <cstdio>
struct S {};
struct A {
  A(const S &);
  virtual void f() const = 0;
};
struct O {};
O &operator<<(O &o, const A &);
void g(const A &);
template <class T> auto pr(O *os, const T &v, int) -> decltype((void)(*os << v)) {
  *os << v;
  std::printf("stream");
}
template <class T> void pr(O *, const T &, long) { std::printf("fallback"); }
template <class T> auto call(const T &v, int) -> decltype(g(v)) { g(v); }
template <class T> void call(const T &, long) { std::printf(" call-fallback"); }
int main() {
  O o;
  S s;
  pr(&o, s, 0);
  call(s, 0);
  std::printf("\n");
}
