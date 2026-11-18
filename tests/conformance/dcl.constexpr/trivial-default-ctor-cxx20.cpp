// [dcl.constexpr]/4 since C++20 (P1331R2, [diff.cpp17.dcl.dcl]): a
// constexpr constructor need not initialize every member, so the
// implicit default constructor of a class with an uninitialized scalar
// member is constexpr, and default-initializing one in a constant
// evaluation is fine as long as no indeterminate value is read.
//@ args: -std=c++20
//@ strict-diags

struct Scalar { int a; };
constexpr int scalar(int i) { Scalar t; t.a = i; return t.a; }
static_assert(scalar(1) == 1);

struct Array { int a[2]; };
constexpr int array(int i) { Array t; t.a[0] = i; return t.a[0]; }
static_assert(array(2) == 2);

union Union { int a; char c; };
constexpr int unite(int i) { Union t; t.a = i; return t.a; }
static_assert(unite(3) == 3);

struct Anon { union { int a; char c; }; };
constexpr int anon(int i) { Anon t; t.a = i; return t.a; }
static_assert(anon(4) == 4);

struct Base { int b; };
struct Derived : Base {};
constexpr int base(int i) { Derived t; t.b = i; return t.b; }
static_assert(base(5) == 5);

struct Defaulted { int a; Defaulted() = default; };
constexpr int deflt(int i) { Defaulted t; t.a = i; return t.a; }
static_assert(deflt(6) == 6);

template <class T> struct Templ { T a; };
constexpr int templ(int i) { Templ<int> t; t.a = i; return t.a; }
static_assert(templ(7) == 7);

struct Holder {
  constexpr Holder() : n(8) {}
  Scalar s;
  int n;
};
static_assert(Holder().n == 8);
