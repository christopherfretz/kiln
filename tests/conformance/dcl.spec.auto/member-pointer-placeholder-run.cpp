// [dcl.type.auto.deduct]/4, [temp.deduct.call]/2-4, [dcl.mptr]: a
// placeholder under a pointer-to-member declarator is deduced as for a
// call with `auto` as the template parameter. The class matches exactly
// and the pointee recurses: data members, member functions (cv- and
// ref-qualified ones too), `auto` in a member function's trailing return
// type, nested member pointers, and the written cv on the placeholder. The
// same deduction serves deduced return types, `new auto Q::*(e)`,
// `template <auto Q::* P>`, and templated entities: a class template's
// static data member (its initializer deferred, [temp.inst]/4) and a
// function template's locals deduce at instantiation.
//@ expect: run
//@ stdout: 7 8 7 10 11 12 7 7 9 7 7 7 13 8 1 8 22
extern "C" int printf(const char*, ...);
struct Q {
  int m = 7;
  int n = 8;
  int Q::* mp = &Q::m;
  Q *self = this;
  int f(int x) { return m + x; }
  int g() const & { return 11; }
  int h() && { return 12; }
};
template <class C> concept C4 = sizeof(C) == 4;
auto Q::* p = &Q::m;
const auto Q::* cp = &Q::n;
auto Q::* const kp = &Q::m;
auto (Q::* f1)(int) = &Q::f;
auto (Q::* f2)() const & = &Q::g;
auto (Q::* f3)() && = &Q::h;
auto (Q::* f4)(int) -> auto = &Q::f;
auto *Q::* sp = &Q::self;
auto Q::* Q::* np = &Q::mp;
C4 auto Q::* c4 = &Q::m;
static_assert(__is_same(decltype(p), int Q::*));
static_assert(__is_same(decltype(cp), const int Q::*));
static_assert(__is_same(decltype(kp), int Q::* const));
static_assert(__is_same(decltype(f1), int (Q::*)(int)));
static_assert(__is_same(decltype(f2), int (Q::*)() const &));
static_assert(__is_same(decltype(f3), int (Q::*)() &&));
static_assert(__is_same(decltype(f4), int (Q::*)(int)));
static_assert(__is_same(decltype(sp), Q *Q::*));
static_assert(__is_same(decltype(np), int Q::* Q::*));
static_assert(__is_same(decltype(c4), int Q::*));
auto Q::* ret() { return &Q::n; }
auto ret2() -> auto (Q::*)(int) { return &Q::f; }
static_assert(__is_same(decltype(ret()), int Q::*));
static_assert(__is_same(decltype(ret2()), int (Q::*)(int)));
template <auto Q::* P> int get(const Q &q) { return q.*P; }
template <auto (Q::* P)(int)> int call(Q &q, int x) { return (q.*P)(x); }
template <class C, auto C::* P> struct V { static constexpr auto value = P; };
static_assert(__is_same(decltype(V<Q, &Q::n>::value), int Q::* const));
template <class T> struct X { static constexpr auto (T::* v) = &T::n; };
template <class T> int tf(T &x) {
  auto T::* a = &T::m;
  auto (T::* fp)(int) = &T::f;
  auto *b = new auto T::*(&T::m);
  int r = x.*a + x.**b + (x.*fp)(1);
  delete b;
  return r;
}
constexpr auto Q::* cx = &Q::m;
constexpr int via_constexpr() { Q q; return q.*cx; }
static_assert(via_constexpr() == 7);
int main() {
  Q q;
  const Q cq;
  auto *pp = new auto Q::*(&Q::m);
  printf("%d %d %d %d %d %d ", q.*p, q.*cp, q.*kp, (q.*f1)(3), (cq.*f2)(),
         (Q().*f3)());
  printf("%d %d %d %d ", (q.*sp)->m, q.*(q.*np), (q.*f4)(2), q.*c4);
  printf("%d %d %d ", q.**pp, get<&Q::m>(q), call<&Q::f>(q, 6));
  printf("%d %d %d %d\n", q.*ret(), V<Q, &Q::n>::value == &Q::n, q.*X<Q>::v,
         tf(q));
  delete pp;
}
