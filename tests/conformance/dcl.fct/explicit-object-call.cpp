// [dcl.fct]/6, [over.match.funcs.general]/4, [expr.unary.op]/3.1: an
// explicit object member function takes the object expression as its
// first argument (converted like any argument, also to a base or by value,
// and deducing `Self` for a template), is called through `.` and `->` and
// in constant evaluation, and `&C::f` is an ordinary pointer to function
//@ expect: run
//@ strict-diags
extern "C" int printf(const char*, ...);
struct B { int v = 1; };
struct S : B {
  int x = 5;
  int get(this const S& self) { return self.x; }
  int add(this S self, int d) { return self.x + d; }
  template <class Self> auto&& fwd(this Self&& self) { return static_cast<Self&&>(self).x; }
  int base(this const B& b) { return b.v; }
  int tw(this S& s) { return s.get() * 2; }
};
template <class T> struct V {
  T t{};
  template <class Self, class F>
  constexpr decltype(auto) visit(this Self&& self, F&& f) { return f(static_cast<Self&&>(self).t); }
};
struct Id { constexpr int operator()(int a) const { return a + 1; } };
int main() {
  S s; const S& cs = s; S* p = &s;
  int r = s.get() + cs.get() + p->add(2) + s.fwd() + S{}.fwd() + s.base() + p->tw();
  V<int> v; v.t = 41;
  constexpr int k = V<int>{3}.visit(Id{});
  static_assert(k == 4);
  int (*f)(const S&) = &S::get;
  auto g = &S::add;
  printf("%d %d %d %d\n", r, v.visit(Id{}), f(s), g(s, 3));
  return 0;
}
