// [class.union.anon]/1, [expr.ref]/2, /7, [temp.inst]: accesses to the
// members of anonymous unions and structs in instantiated function
// bodies, each a full-expression that the bit-field truncation check
// and -Winteger-overflow fold: through `->` and `.`, nested anonymous
// members, a base's anonymous member, a static data member's, a prvalue
// object's, compound assignment, an implicit and explicit `this`,
// lambdas, a deduced object type, `const` and `volatile` objects, and
// constant evaluation (the `static_assert`s). The instantiation dropped
// the conversions of the object (the pointer's lvalue-to-rvalue
// conversion, a derived-to-base cast), on which the constant evaluator
// crashed. Reduced from LLVM 19 (`TypePropertyCache::ensure`).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct T { union { unsigned a : 2; int other; }; };
struct N { union { struct { unsigned a : 2; int y; }; int other; }; };
struct D : T {};
struct M {
  union { unsigned a : 2; int other; };
  static T s;
  void f(M *o);
  template <class P> void g(M *o) { a = o->a; this->a = o->a; other = o->other + 1; }
};
struct U { union { int x; long y; }; constexpr U(int v) : x(v) {} };
T make();

template <class P> void arrow(T *t, T *ct) { t->a = ct->a; }
template <class P> void dot(T &t, T &ct) { t.a = ct.a; }
template <class P> void compound(T *t, T *ct) { t->a += ct->a; t->other *= ct->other; }
template <class P> void nested(N *t, N *ct) { t->a = ct->a; t->y = ct->y + 1; }
template <class P> void nested_dot(N &t, N &ct) { t.a = ct.a; }
template <class P> void base_arrow(D *t, D *ct) { t->a = ct->a; }
template <class P> void base_dot(D &t, D &ct) { t.a = ct.a; }
template <class P> void static_member(T *t) { t->a = M::s.a; }
template <class P> void prvalue(T *t) { t->a = make().a; }
template <class P> void lambda_ref(T *t, T *ct) { auto l = [&] { t->a = ct->a; }; l(); }
template <class P> void lambda_copy(T *t, T *ct) { [=] { t->a = ct->a; }(); }
template <class P> unsigned deduced(P *t) { return t->a; }
template <class P> void deduced2(P *t, P *ct) { t->a = ct->a; }
template <class P> void other(T *t, T *ct) { t->a = ct->other; ct->other = t->a; }
template <class P> struct H { T *t; void f() { t->a = t->a; } };
template <class P> void through_deref(T *const *pp) { (*pp)->a = (*pp)->a; }
template <class P> void array(T (&arr)[2]) { arr->a = arr[1].a; }
template <class P> void cv(volatile T *t, const T *ct) { t->a = ct->a; }
void M::f(M *o) { g<int>(o); }

template <class P> constexpr int cx_ptr(int v) {
  U u(v);
  U *p = &u;
  p->x += 1;
  return p->x + u.x;
}
static_assert(cx_ptr<int>(3) == 8);
template <class P> constexpr int cx_local(int v) {
  struct L { union { int x; long y; }; } l{v};
  L *p = &l;
  p->x += 1;
  return p->x;
}
static_assert(cx_local<int>(3) == 4);
template <class P> constexpr unsigned cx_bits(unsigned v) {
  T t{};
  T *p = &t;
  p->a = v;
  T *q = p;
  return q->a;
}
static_assert(cx_bits<int>(7) == 3);

void use(T *t, N *n, D *d, T (&arr)[2], T **pp) {
  arrow<int>(t, t);
  dot<int>(*t, *t);
  compound<int>(t, t);
  nested<int>(n, n);
  nested_dot<int>(*n, *n);
  base_arrow<int>(d, d);
  base_dot<int>(*d, *d);
  static_member<int>(t);
  prvalue<int>(t);
  lambda_ref<int>(t, t);
  lambda_copy<int>(t, t);
  deduced(t);
  deduced(d);
  deduced2(t, t);
  deduced2(d, d);
  other<int>(t, t);
  H<int>{t}.f();
  through_deref<int>(pp);
  array<int>(arr);
  cv<int>(t, t);
}
