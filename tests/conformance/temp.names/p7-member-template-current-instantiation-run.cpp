// [temp.names]/7.3.2, [basic.lookup.qual.general]/3, [temp.dep.type]/7:
// a member template named after `.`/`->` on an object of the current
// instantiation (through `this->`, an implicit `this`, a pointer,
// reference or object of type `C` / `C<T>`, from a nested class, in a
// generic lambda, a trailing return type, a noexcept-specifier and a
// default member initializer) is looked up where it appears, so `<`
// starts a template argument list without `template`. The same holds with
// a qualifier naming the current instantiation or a non-dependent base,
// also when the name is overloaded (templates and non-templates), and for
// static member templates and member variable templates.
// Found running GCC's C++ testsuite (WP-1530).
//@ expect: run
//@ strict-diags
struct NB {
  template <class U> int nb() { return 3; }
  int nb(int) { return 0; }
};
template <class T> struct C : NB {
  template <class U> int f() { return 0; }
  int f(int) { return 100; }
  template <class U> static int sf() { return 1; }
  template <class U> static constexpr int vt = 2;
  int g(C *p, C &r, C<T> c) {
    int a = this->f<int>() + f<int>();
    int b = p->f<int>() + r.f<int>() + c.f<int>();
    int d = p->C::f<int>() + this->C<T>::f<int>() + this->NB::nb<int>() + nb<char>();
    int e = this->sf<int>() + this->vt<int>;
    auto l = [this](auto x) { return this->f<decltype(x)>() + f<int>(); };
    return a + b + d + e + l(1);
  }
  struct N {
    int h(C *p) { return p->f<int>() + p->C::f<int>(); }
  };
  auto t() -> decltype(this->f<int>()) { return 0; }
  void n() noexcept(noexcept(this->f<int>())) {}
  int m = this->f<long>() + f<long>();
};
int main() {
  C<int> c;
  c.n();
  return c.g(&c, c, c) + C<int>::N().h(&c) + c.t() + c.m == 9 ? 0 : 1;
}
