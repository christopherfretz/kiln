// [temp.friend]/1 and /5: a friend naming a member of a class template
// specialization `A<int>` (or of a class nested in it) is written
// without a head for `A<int>`; a member template's own head is not
// one (an explicit specialization `A<char>` is an ordinary class)
// (clang 22 and GCC 16.1 accept;
// temp.friend/member-of-specialization-head-not-empty.cpp).
// Found running GCC's C++ testsuite.
//@ expect: run
//@ stdout: 1 2 3 4

extern "C" int printf(const char *, ...);

class B;
template <class T> struct A {
  A(int *p);
  void f(int &out);
  struct N {
    void h(int &out);
  };
};
template <> struct A<char> {
  template <class U> void g(U, int &out);
};

class B {
  static int secret() { return 1; }
  friend A<int>::A(int *);
  friend void A<int>::f(int &);
  friend void A<int>::N::h(int &);
  template <class U> friend void A<char>::g(U, int &);
  template <class T> friend struct A;
};

template <class T> A<T>::A(int *p) { *p = B::secret(); }
template <class T> void A<T>::f(int &out) { out = B::secret() + 1; }
template <class T> void A<T>::N::h(int &out) { out = B::secret() + 2; }
template <class U> void A<char>::g(U u, int &out) { out = B::secret() + int(u); }

int main() {
  int a = 0, b = 0, c = 0, d = 0;
  A<int> x(&a);
  x.f(b);
  A<int>::N().h(c);
  A<char>().g(3, d);
  printf("%d %d %d %d\n", a, b, c, d);
}
