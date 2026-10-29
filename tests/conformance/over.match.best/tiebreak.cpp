// [over.match.best]/2: the best viable function is better for at least
// one argument and not worse for any; the return-conversion tie-breaker
// (/2.2); ambiguity when no function is better.
//@ expect: compile-ok
struct A {};
struct B : A {};
struct C : B {};

char f(int, long);
long f(long, int);
static_assert(__is_same(decltype(f(1, 2L)), char));
static_assert(__is_same(decltype(f(1L, 2)), long));

// Derived-to-base ranking: the nearer base wins ([over.ics.rank]/4.4).
int g(A *);
long g(B *);
C *pc();
static_assert(__is_same(decltype(g(pc())), long));
int h(A &);
long h(B &);
C &rc();
static_assert(__is_same(decltype(h(rc())), long));

// /2.2: user-defined conversions by the same conversion function... no;
// different conversion functions: the better second standard conversion
// of the returned value decides in an initialization.
struct X {
  operator int();
  operator double();
};
X x();
int i = x();
double d = x();

// Exact match beats promotion beats conversion.
char k(int);
long k(double);
short k(char);
static_assert(__is_same(decltype(k('a')), short));
static_assert(__is_same(decltype(k((short)1)), char));
static_assert(__is_same(decltype(k(1.0f)), long));

// Pointer to bool is worse than other pointer conversions.
char p(bool);
long p(void *);
static_assert(__is_same(decltype(p((int *)0)), long));

// Qualification adjustment: the less-qualified binding wins.
char q(const int &);
long q(int &);
int &ri();
const int &cri();
static_assert(__is_same(decltype(q(ri())), long));
static_assert(__is_same(decltype(q(cri())), char));

// Rvalue reference binding of an rvalue beats an lvalue reference.
char r(const int &);
long r(int &&);
static_assert(__is_same(decltype(r(1)), long));
static_assert(__is_same(decltype(r(ri())), char));
