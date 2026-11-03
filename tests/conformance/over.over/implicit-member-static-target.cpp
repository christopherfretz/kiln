// [over.over]/1 with [class.mfct.non.static]/2 (WP-402): an unqualified
// member name inside a member function that names an overload set of a
// non-static and a static member function selects the static one when the
// target is a pointer to function: a constructor argument, a braced base
// initializer, an initialized pointer, `&f`, and the same in a class
// template whose `this` is dependent (libc++ <format> `__allocating_buffer`
// passing `__prepare_write` to its `__output_buffer` base; every
// std::print test failed with "no matching constructor").
//@ expect: run
//@ strict-diags
struct B {
  using F = int (*)(B &, int);
  B(F f) : f_(f) {}
  B(void *, F f, int) : f_(f) {}
  F f_;
};
struct D : B {
  D() : B(step) {}
  int step(int n) { return n + 1; }
  static int step(B &b, int n) { return static_cast<D &>(b).step(n) * 2; }
  int local() {
    B::F p = step;
    B::F q = &step;
    auto *r = static_cast<B::F>(step);
    return p(*this, 1) + q(*this, 2) + r(*this, 3);
  }
};
template <class C> struct T : B {
  T() : B{buf_, step, 0} {}
  C buf_[2];
  int step(int n) { return n + 10; }
  static int step(B &b, int n) { return static_cast<T &>(b).step(n) * 3; }
  int local() {
    B::F p = step;
    return p(*this, 1);
  }
};
int main() {
  D d;
  T<char> t;
  int a = d.f_(d, 4);   // (4 + 1) * 2 = 10
  int b = d.local();    // 4 + 6 + 8 = 18
  int c = t.f_(t, 2);   // (2 + 10) * 3 = 36
  int e = t.local();    // 33
  return a == 10 && b == 18 && c == 36 && e == 33 ? 0 : 1;
}
