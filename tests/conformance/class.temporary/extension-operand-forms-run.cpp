// [class.temporary]/6: a reference bound to a glvalue that designates a
// temporary or a subobject of one through the listed operand forms
// extends the whole complete temporary to the reference's lifetime:
// parentheses, subscripting of an array operand (either operand of `[]`),
// `.` naming a non-static data member of non-reference type, `.*` with a
// pointer to data member, const_cast/static_cast/dynamic_cast/
// reinterpret_cast to a reference without a user-defined conversion
// (derived-to-base, base-to-derived, dynamic, reinterpretation), both
// arms of a glvalue conditional (only the chosen one exists), and the
// right operand of a comma; each also nested two deep, for `const&` and
// `&&`, for block-scope automatic, static and thread-local references and
// for a reference member of an aggregate. Each `uN` must print before
// `~AN`. Found running GCC's C++ testsuite (`.*` destroyed the object
// before the reference's scope ended).
//@ expect: run
//@ strict-diags
//@ xfail(gcc): GCC 16.1 destroys the temporary early through a nested member access of a `.*`, a conditional or a comma, a reversed subscript and a dynamic_cast
//@ stdout: u1 ~A1 u2 ~A2 u3 ~A3 u4 ~A4 u5 ~A5 u6 ~A6 u7 ~A7 u8 ~A8 u9 ~A9 u10 ~A10 u11 ~A11 u12 ~A12 u13 ~A13 u14 ~A14 u15 ~A15 u16 ~A16 u17 ~A17 u18 ~A18 u19 ~A19 u20 ~A20 u21 ~A21 ~A21 s1 s2 t1 | end ~A31 ~A30 |
extern "C" int printf(const char*, ...);
struct Last {
  ~Last() { printf("|\n"); }
} last;
struct B { int v; int w; };
struct A {
  B b;
  B arr[2];
  int i;
  A(int x) : b{x, x}, arr{{x, x}, {x, x}}, i(x) {}
  virtual ~A() { printf("~A%d ", i); }
};
struct D : A {
  using A::A;
};
struct R {
  const B& r;
};
struct RR {
  R inner;
  int&& j;
};
int use(int n, int v) {
  printf("u%d ", n);
  return v;
}

void statics(int n) {
  static const B& s = A(30).*(&A::b);
  thread_local B&& t = static_cast<B&&>((A(31).arr)[1]);
  printf("s%d ", n + s.v - 30);
  if (n == 2)
    printf("t%d ", t.v - 30);
}

bool c = true;

int main() {
  { const B& r = A(1).*(&A::b); use(1, r.v); }
  { B&& r = A(2).*(&A::b); use(2, r.v); }
  { const int& r = (A(3).*(&A::b)).v; use(3, r); }
  { const int& r = A(4).arr[1].v; use(4, r); }
  { int&& r = 1[A(5).arr].w; use(5, r); }
  { const B& r = (A(6).b); use(6, r.v); }
  { const int& r = (A(7).b).v; use(7, r); }
  { const A& r = static_cast<const A&>(D(8)); use(8, r.i); }
  { const D& r = static_cast<const D&>(static_cast<const A&>(D(9))); use(9, r.i); }
  { const D& r = dynamic_cast<const D&>(static_cast<const A&>(D(10))); use(10, r.i); }
  { const unsigned& r = reinterpret_cast<const unsigned&>(A(11).i); use(11, int(r)); }
  { int&& r = const_cast<int&&>(A(12).i); use(12, r); }
  { int&& r = const_cast<int&&>(A(13).*(&A::i)); use(13, r); }
  { const B& r = ((void)0, A(14).b); use(14, r.v); }
  { const int& r = ((void)0, ((void)0, A(15).b).v); use(15, r); }
  { const B& r = c ? A(16).b : A(99).b; use(16, r.v); }
  { B&& r = !c ? static_cast<B&&>(A(98).b) : static_cast<B&&>(A(17).b); use(17, r.v); }
  { const int& r = (c ? A(18).b : A(97).b).w; use(18, r); }
  { R r{A(19).b}; use(19, r.r.v); }
  { R r{A(20).*(&A::b)}; use(20, r.r.v); }
  { RR r{{A(21).arr[0]}, static_cast<int&&>(A(21).i)}; use(21, r.inner.r.v + r.j - 21); }
  statics(1);
  statics(2);
  printf("| end ");
}
