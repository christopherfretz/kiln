// [temp.friend]/1, /5: `template<class U> friend X<U>::X(U);` befriends the
// constructor `X<U>::X(U)` of every specialization of `X` (GCC 16.1
// accepts; found running GCC's C++ testsuite). The sibling
// friend-constructor-of-every-specialization-denied.cpp keeps the other
// constructors private.
//@ skip(clang): clang 22 rejects a friend constructor template of a class template
//@ expect: run
//@ stdout: 3
extern "C" int printf(const char *, ...);
template <class T> struct X { X(T); };
template <class T> class Y {
  template <class U> friend X<U>::X(U);
  static int x;
};
template <class T> int Y<T>::x = 3;
template <class T> X<T>::X(T) { printf("%d\n", Y<char>::x); }
int main() { X<int> a(1); }
