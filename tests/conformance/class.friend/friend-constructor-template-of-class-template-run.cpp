// [temp.friend]/1: `template<class U> friend X<U>::X(U);` befriends the
// constructor `X<U>::X(U)` of every specialization of `X` (GCC 16.1
// accepts; found running GCC's C++ testsuite). kiln accepts the
// declaration but befriends nothing.
//@ skip(clang): clang 22 rejects a friend constructor template of a class template
//@ xfail(kiln): WP-1514: a friend template naming a dependent class's constructor befriends no specialization's constructor
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
