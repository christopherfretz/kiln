//@ expect: run
//@ stdout: 3 4 1 4
// [namespace.udecl]/1, /19: a using-declaration names a base's conversion
// functions in the derived class, with the using-declaration's access, so
// a conversion through a private base made public by `using` (and by a
// further derived class) is accessible (folly relaxed_atomic).
#include <cstdio>
template <class T> struct B {
  T v;
  operator T() const { return v; }
  explicit operator bool() const { return v != 0; }
};
template <class T> struct I : private B<T> {
  using base = B<T>;
  I(T x) : base{x} {}
  using base::operator T;
  using base::operator bool;
};
template <class T> struct R : I<T> { using I<T>::I; };
struct C : private B<long> {
  C() : B<long>{4} {}
  using B<long>::operator long;
};
int main() {
  R<unsigned> r(3);
  unsigned v = r;
  long w = C();
  bool b = static_cast<bool>(r);
  switch (r) { case 3: break; default: return 1; }
  std::printf("%u %ld %d %d\n", v, w, b, r + 1);
}
