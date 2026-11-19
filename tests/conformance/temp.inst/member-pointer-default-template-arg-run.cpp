// [temp.inst]/5, [basic.def.odr]/4: a member function named only by a
// default template argument (`template <auto C = &X<T>::f>`) is odr-used
// where the specialization that uses the default substitutes it, so the
// member of a class template specialization is instantiated and the
// program links. The default of a member template of a class template,
// spelled with the injected-class-name or `X<T>::`, the candidate passed
// on to another member template, a non-template class, and a
// namespace-scope function template whose default names a member of a
// class template specialization. Reduced from real code (a signal mixin
// binding a member function as its default candidate; WP-1724).
//@ expect: run
//@ stdout: 1 2 3 4 5 6
extern "C" int printf(const char *, ...);

template <typename T> struct mixin {
  void emplace_element(int v) { n += v; }
  template <auto Candidate = &mixin::emplace_element> void bind() {
    (this->*Candidate)(1);
  }
  int n = 0;
};

template <typename T> struct qualified {
  void emplace_element(int v) { n += v; }
  template <auto Candidate = &qualified<T>::emplace_element> void bind() {
    (this->*Candidate)(2);
  }
  int n = 0;
};

template <typename T> struct delegate {
  template <auto Candidate, typename U> void connect(U &u) { (u.*Candidate)(3); }
};

template <typename T> struct forwarding {
  void emplace_element(int v) { n += v; }
  template <typename Clazz, auto Candidate = &forwarding::emplace_element>
  forwarding &on_construct() {
    delegate<Clazz> d;
    d.template connect<Candidate>(*this);
    return *this;
  }
  int n = 0;
};

struct plain {
  void f(int v) { n += v; }
  template <auto Candidate = &plain::f> void bind() { (this->*Candidate)(4); }
  int n = 0;
};

template <typename T> struct holder {
  void g() { n += 5; }
  void h() { n += 6; }
  int n = 0;
};

template <auto C = &holder<int>::g> void call_default(holder<int> &x) {
  (x.*C)();
}

template <typename T, auto C = &holder<T>::h> void call_dependent(holder<T> &x) {
  (x.*C)();
}

int main() {
  mixin<int> a;
  a.bind();
  qualified<int> b;
  b.bind();
  forwarding<int> c;
  c.on_construct<char>();
  plain d;
  d.bind();
  holder<int> e;
  call_default(e);
  holder<long> f;
  call_dependent(f);
  printf("%d %d %d %d %d %d\n", a.n, b.n, c.n, d.n, e.n, f.n);
}
