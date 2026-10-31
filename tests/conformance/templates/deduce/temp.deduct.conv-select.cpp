// [temp.deduct.conv]/1-4, [over.match.conv]: a conversion function
// template deduces its conversion type from the required type (after
// decay and cv-stripping of P and A); a non-template conversion wins a
// tie, a more specialized template is preferred, and an explicit
// template only serves direct-initialization.
//@ expect: compile-ok
struct A {
  template <class T> constexpr operator T() const { return T(42); }
};
constexpr int x = A();
constexpr double y = A();
static_assert(x == 42 && y == 42.0);
struct B {
  template <class T> constexpr operator T*() const { return nullptr; }
  constexpr operator int() const { return 7; }
};
constexpr int* p = B();
constexpr int i = B();
static_assert(p == nullptr && i == 7);
constexpr long l = B();
static_assert(l == 7);
int f(int);
long g = f(A());
struct C { template <class T> operator const T&() const; };
const int& r = C();
bool b = A() ? true : false;
struct D { template <class T> explicit operator T() const { return T(); } };
int q = static_cast<int>(D());
struct E {
  template <class T> constexpr operator T() const { return T(1); }
  template <class T> constexpr operator T*() const { return nullptr; }
};
constexpr int* pe = E();
static_assert(pe == nullptr);
