// [over.ics.list]: implicit conversion sequences of braced-init-list
// arguments: scalar from a one-element list, class by constructor,
// aggregates, narrowing.
//@ expect: compile-ok
//@ diag: 10:36 warning "braces around scalar initializer"
//@ diag: 11:36 warning "braces around scalar initializer"
//@ diag: 18:36 warning "braces around scalar initializer"
char f(int);
long f(double);
static_assert(__is_same(decltype(f({1})), char));
static_assert(__is_same(decltype(f({1.0})), long));
struct P {
  int x, y;
};
char g(P);
long g(int);
static_assert(__is_same(decltype(g({1, 2})), char));
static_assert(__is_same(decltype(g({1})), long));
struct S {
  S(int, int);
};
char h(S);
long h(long);
static_assert(__is_same(decltype(h({1, 2})), char));
char e(int);
static_assert(__is_same(decltype(e({})), char));
