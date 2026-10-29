// A parameter is in scope in the rest of its function declarator
// ([basic.scope.param], [dcl.fct]/6): the trailing return type, the
// exception specification and later default arguments name it (not an
// outer entity), unevaluated. In a member function's trailing return
// type `this` may appear, as cv C* ([expr.prim.this]/2).
//@ expect: compile-ok
//@ strict-diags
struct P { int x; double y; };
extern P pv;
auto f(P& t) -> decltype(t.x);
auto g(int a) -> decltype(a + 1);
auto h(int a, long b) -> decltype(a + b) { return a + b; }
auto s(char c) -> char(*)[sizeof(c) + 1];
static_assert(__is_same(decltype(f(pv)), int));
static_assert(__is_same(decltype(g(1)), int));
static_assert(__is_same(decltype(h(1, 2)), long));
static_assert(__is_same(decltype(s('a')), char(*)[2]));
int a = 0;
auto shadow(double a) -> decltype(a);
static_assert(__is_same(decltype(shadow(1)), double));
void n(int a) noexcept(noexcept(a + 1));
static_assert(noexcept(n(1)));
void d(int a, int b = sizeof(a));
void d2(int a, decltype(a) b = 0);
auto lp(int (*fp)(int q), int q) -> decltype(fp(q));
static_assert(__is_same(decltype(lp(nullptr, 1)), int));
struct S {
  int m;
  auto get(int k) -> decltype(k + m);
  auto self() -> decltype(this);
  auto cself() const -> decltype(this);
  auto mm() -> decltype((m));
  auto pm(int S::*p) -> decltype(this->*p);
};
extern S *sp;
static_assert(__is_same(decltype(sp->get(1)), int));
static_assert(__is_same(decltype(sp->self()), S*));
static_assert(__is_same(decltype(sp->cself()), const S*));
static_assert(__is_same(decltype(sp->mm()), int&));
static_assert(__is_same(decltype(sp->pm(&S::m)), int&));
template<class T> auto tf(T t) -> decltype(t.y);
template<class T> struct C {
  T v;
  auto get(T t) -> decltype(t + v);
  auto twice(T t) -> decltype(t * 2);
  auto self() const -> decltype(this);
};
extern C<int> *ci;
extern C<double> *cd;
extern C<long> *cl;
static_assert(__is_same(decltype(ci->get(1)), int));
static_assert(__is_same(decltype(cd->twice(1)), double));
static_assert(__is_same(decltype(cl->self()), const C<long>*));
template<class T> struct D {
  auto get(T &t) -> decltype(t.x);
  auto first(T &t, decltype(t.y) d = 0) -> decltype(d);
};
extern D<P> *dp;
static_assert(__is_same(decltype(dp->get(pv)), int));
static_assert(__is_same(decltype(dp->first(pv)), double));
