// [class.conv.fct]/1 (P0847): a conversion function with an explicit
// object parameter is emitted as an explicit-object member function: the
// object is an ordinary first parameter (a reference or, for `this H`, a
// value), the mangled name carries `H` (`_ZNH1ScviERKS_`), a deduced
// `this auto&&` / `this Self&&` conversion is instantiated per object
// type, and the conversion is called with the object as its first
// argument (no `this` adjustment, no temporary for an lvalue object).
struct S { int v = 3; operator int(this const S& self) { return self.v; } };
struct R { operator int(this R&) { return 1; } operator int(this R&&) { return 2; } };
struct G {
  int v;
  template <class Self> operator int(this Self&& self) {
    return __is_lvalue_reference(Self) ? self.v : -self.v;
  }
};
struct T { template <class U> operator U(this const T&) { return U(5); } };
struct E { explicit operator bool(this E) { return true; } };
struct B { int v; operator int(this auto&& self) { return self.v; } };
struct D : B {};
struct H { int k = 11; operator int(this H self) { return self.k; } };
struct P { int v; operator int*(this P& self) { return &self.v; } };
int use_s(S& s) { return s; }
int use_r(R& r) { int a = r; int b = R{}; return a + b; }
int use_g(G& g) { int a = g; int b = G{4}; return a + b; }
long use_t() { long l = T{}; double d = T{}; return l + long(d); }
bool use_e(E e) { return e ? true : false; }
int use_b(D& d) { return B{8} + 1 + d; }
int use_h(H h) { return h; }
int* use_p(P& p) { return p; }
