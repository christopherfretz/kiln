// [temp.friend]/1: in a class template, a friend function template
// specialization that depends on no template parameter and matches a
// function template befriends that specialization in every
// specialization of the class template; a dependent one is matched as
// each specialization is instantiated (clang 22 and GCC 16.1 accept;
// temp.friend/friend-spec-no-match-in-template.cpp).
// Found running GCC's C++ testsuite.
//@ expect: run
//@ args: -std=c++20
//@ stdout: 1 2 3 4

extern "C" int printf(const char *, ...);

template <class T> class B;
struct A {
  template <int N> static int foo(B<int> &);
};
template <class T> int g(T, B<int> &);
template <class T> int h(B<T> &);

template <class T> class B {
  int v = 1;
  friend int A::foo<0>(B<int> &);
  friend int g<>(long, B<int> &);
  friend int h<>(B<T> &);
  friend int h<T *>(B<T *> &);
};

template <int N> int A::foo(B<int> &b) { return b.v + N; }
template <class T> int g(T t, B<int> &b) { return b.v + int(t); }
template <class T> int h(B<T> &b) { return b.v + 2; }

int main() {
  B<int> b;
  B<char> c;
  B<char *> d;
  printf("%d %d %d %d\n", A::foo<0>(b), g(1L, b), h(c) + 0, h(d) + 1);
}
