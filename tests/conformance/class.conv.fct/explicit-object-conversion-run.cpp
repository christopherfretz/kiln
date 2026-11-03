// [class.conv.fct]/1, [over.ics.user] (P0847): explicit-object conversion
// functions at run time: the object is passed as the first argument (by
// reference or by value), the lvalue/rvalue and template overloads pick
// by the object's value category, a derived-class object converts through
// the base's deduced `this auto&&`, and a conversion to a pointer returns
// the object's member address. Matches clang 19 and 22.
//@ expect: run
//@ stdout: 3 1 2 4 -4 5 5 1 8 6 11 5
extern "C" int printf(const char*, ...);
struct S { int v = 3; operator int(this const S& self) { return self.v; } };
struct R { operator int(this R&) { return 1; } operator int(this R&&) { return 2; } };
struct G { int v; template <class Self> operator int(this Self&& self) { return __is_lvalue_reference(Self) ? self.v : -self.v; } };
struct T { template <class U> operator U(this const T&) { return U(5); } };
struct E { explicit operator bool(this E) { return true; } };
struct B { int v; operator int(this auto&& self) { return self.v; } };
struct D : B {};
struct H { int k = 11; operator int(this H self) { return self.k; } };
struct P { int v; operator int*(this P& self) { return &self.v; } };
int main() {
  S s; R r; G g{4}; H h; P p{5};
  int a = s, b = r, c = R{}, d = g, e = G{4};
  long f = T{}; double fd = T{};
  int i = D{{6}};
  int* ip = p;
  printf("%d %d %d %d %d %ld %g %d %d %d %d %d\n", a, b, c, d, e, f, fd, E{} ? 1 : 0, B{8} + 0, i, int(h), *ip);
}
