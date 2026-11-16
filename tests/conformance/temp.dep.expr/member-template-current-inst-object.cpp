// [temp.dep.expr]/5, [temp.dep.type]/1: an object whose type is the
// current instantiation is type-dependent, so a call of a member function
// template through it (`o.f<int>()`, a capture `[o]`, `[*this]`, `[&]`
// with `p->f<int>()`) is resolved when the member is instantiated. A
// const object calling a non-const member template is not an error in a
// member that is never instantiated. Found running GCC's C++ testsuite
// (WP-1472).
//@ expect: run
//@ std: c++20
template <class T> struct O {
  int v = 0;
  template <class U> int f() { return v + int(sizeof(U)); }
  template <class U> int k(U u) const { return v + int(u); }

  // Never instantiated: `o` is const in the lambda (and in e).
  void a(O o) { [o] { o.f<int>(); }; }
  void b(O *p) { [&] { p->f<int>(); }; }
  void c() { [*this] { f<int>(); }; }
  void e(const O o) { o.f<int>(); }

  // Instantiated.
  int g1() { return [outer = *this]() mutable { return outer.template f<char>(); }(); }
  int g2() { return [this] { return this->f<char>(); }(); }
  int g3() { return [*this]() mutable { return f<char>(); }(); }
  int g4(O *p) { return [&] { return p->f<char>(); }(); }
  int g5(const O &o) const { return [o] { return o.k<int>(2); }(); }
  int g6(O o) { return [&o] { return o.f<char>(); }(); }
  int g7() { auto [w] = *this; return w; }
};

int main() {
  O<int> s;
  s.v = 1;
  if (s.g1() != 2 || s.g2() != 2 || s.g3() != 2 || s.g4(&s) != 2) return 1;
  if (s.g5(s) != 3 || s.g6(s) != 2 || s.g7() != 1) return 2;
}
