// [over.over]/1-5: the address of an overloaded function picks the one whose
// type matches the target: an initialized pointer, a parameter, a cast, a
// return value; pointers to members too.
//@ expect: compile-ok
template <class T, class U> constexpr bool same = __is_same(T, U);
int f(int);
long f(long);
char f(char, char);
int (*p1)(int) = f;
long (*p2)(long) = &f;
char (&r1)(char, char) = f;
void take(long (*)(long));
void use() { take(f); }
auto p3 = static_cast<char (*)(char, char)>(f);
static_assert(same<decltype(p3), char (*)(char, char)>);
int (*ret())(int) { return f; }
void (*pv)(int);
struct A {
  int m(int);
  int m(long) const;
  static int s(int);
  static int s(long);
};
int (A::*pm1)(int) = &A::m;
int (A::*pm2)(long) const = &A::m;
int (*ps)(int) = &A::s;
int (*nf)(int) noexcept(false) = f;
