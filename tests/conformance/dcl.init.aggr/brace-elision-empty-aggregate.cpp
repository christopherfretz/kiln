// [dcl.init.aggr]/2, /16 (WP-325): brace elision never enters an aggregate
// with no elements (an empty class, a class of only unnamed bit-fields, an
// array of bound 0). An unbraced clause for such an element is an error
// (clang and GCC), it is consumed, and the next clause initializes the next
// element. Its own braces (`{}`) initialize it, and elision into a
// non-empty subaggregate is unaffected. The overload viability walk uses
// the same rule ([over.ics.list]/8): `fn({1, 2})` has no viable candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:6 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 25:10 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 26:7 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 28:5 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 28:8 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 29:12 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 30:57 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 43:8 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 50:7 error "initializer for aggregate with no elements requires explicit braces"
//@ diag: 52:14 note "in instantiation of function template specialization 'make<D2>' requested here"
//@ diag: 54:9 error "no matching function for call to 'fn'"
//@ diag: 53:5 note "candidate function not viable: cannot convert initializer list argument to 'D2'"
struct E {};
struct D2 { E e; int i; };
D2 d{1, 2};
D2 dc = {1, 2};
D2 d1{1};
struct F { E e[2]; int x; };
F f{1, 2};
E ea[2] = {1};
struct B { int : 3; }; struct DB { B b; int i; }; DB db{1, 2};

constexpr D2 ok{{}, 2};
static_assert(ok.i == 2);
constexpr F fok{{}, 3};
static_assert(fok.x == 3);

struct P { int a, b; };
struct Q { P p; int i; };
constexpr Q q{1, 2, 3};
static_assert(q.p.a == 1 && q.p.b == 2 && q.i == 3);

template <class T> struct W { T e; int i; };
W<E> w{1, 2};
constexpr W<E> wok{{}, 4};
static_assert(wok.i == 4);
constexpr W<P> wp{5, 6, 7};
static_assert(wp.e.b == 6 && wp.i == 7);

template <class T> constexpr int make() {
  T t{1, 2};
  return 0; }
template int make<D2>();
int fn(D2);
int x = fn({1, 2});
template <class T> constexpr int fine() { T t{{}, 2}; return t.i; }
static_assert(fine<D2>() == 2);
template <class T> constexpr int elided() { T t{1, 2, 3}; return t.p.b + t.i; }
static_assert(elided<Q>() == 5);
