// Mangling differential: class template specializations and their
// members, template arguments of every kind, substitutions.
namespace tl {
template <class T> struct Box {
  T v;
  Box() {}
  ~Box() {}
  T get() const { return v; }
  void set(T, const T &, T *) {}
  static int counter;
  template <class U> struct Inner {
    void f(U, T) {}
  };
};
template <class T> int Box<T>::counter = 1;

template <int N> struct Int {
  int f() { return N; }
};
template <bool B> struct Flag {
  bool f() { return B; }
};
enum Color { red, green = 5 };
enum class Scoped : unsigned char { a = 1, b = 200 };
template <Color C> struct Col {
  int f() { return C; }
};
template <Scoped S> struct Sc {
  int f() { return 0; }
};
template <long L> struct Long {
  long f() { return L; }
};
template <unsigned long long U> struct ULL {
  void f() {}
};
template <char C> struct Ch {
  void f() {}
};
template <class A, class B> struct Pair {
  void f(A, B, Pair *) {}
  void g(Pair<B, A>) {}
};
template <template <class> class TT> struct Tmpl {
  void f(TT<int>) {}
};
template <class... Ts> struct Pack {
  void f() {}
};
template <int... Ns> struct IPack {
  void f() {}
};
int gvar;
template <int *P> struct Ptr {
  void f() {}
};
template <decltype(nullptr) N> struct Np {
  void f() {}
};

void use() {
  Box<int> bi;
  bi.get();
  bi.set(1, 2, nullptr);
  Box<int>::counter = 2;
  Box<const char *> bc;
  bc.get();
  bc.set(nullptr, nullptr, nullptr);
  Box<Box<int>> bb;
  bb.get();
  Box<int>::Inner<double> in;
  in.f(1.0, 2);
  Box<long>::Inner<Box<long>> in2;
  in2.f(Box<long>(), 3);
  Int<0>().f();
  Int<-7>().f();
  Int<42>().f();
  Flag<true>().f();
  Flag<false>().f();
  Col<red>().f();
  Col<green>().f();
  Sc<Scoped::a>().f();
  Sc<Scoped::b>().f();
  Long<-1>().f();
  Long<1000000000000>().f();
  ULL<18446744073709551615ull>().f();
  Ch<'a'>().f();
  Pair<int, double> p;
  p.f(1, 2.0, nullptr);
  p.g(Pair<double, int>());
  Pair<Box<int>, Box<int>> pb;
  pb.f(Box<int>(), Box<int>(), nullptr);
  Tmpl<Box> tb;
  tb.f(Box<int>());
  Pack<> p0;
  p0.f();
  Pack<int, char, Box<int>> p3;
  p3.f();
  IPack<1, 2, 3> ip;
  ip.f();
  Ptr<&gvar>().f();
  Np<nullptr>().f();
}
} // namespace tl

template <class T> struct Top {
  void f(T) {}
  static T sv;
};
template <class T> T Top<T>::sv;
void use_top() {
  Top<int>().f(1);
  Top<tl::Box<int>>().f(tl::Box<int>());
  Top<int *>().f(nullptr);
  Top<int>::sv = 3;
}
