// [temp.friend]/1, [class.friend]/4: `template<class U> friend Z::Z(U);` and
// `template<class U> friend Z::operator U*();` befriend every
// specialization of the constructor template and of the conversion
// function template of `Z` (found running GCC's C++ testsuite). The sibling
// friend-special-member-template-denied.cpp keeps the other members
// private.
//@ expect: run
//@ stdout: 3 3 3
extern "C" int printf(const char *, ...);
struct Z {
  template <class U> Z(U);
  template <class U> operator U*();
};
class Y {
  template <class U> friend Z::Z(U);
  template <class U> friend Z::operator U*();
  static int x;
};
int Y::x = 3;
template <class U> Z::Z(U) { printf("%d ", Y::x); }
template <class U> Z::operator U*() { printf("%d", Y::x); return 0; }
int main() {
  Z a(1), b(2L);
  (void)(char *)a;
  printf("\n");
}
