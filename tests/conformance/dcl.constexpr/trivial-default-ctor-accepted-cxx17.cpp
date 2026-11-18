// [class.ctor]/7 and [dcl.constexpr]/4 in C++17: the implicit default
// constructor is constexpr when it leaves nothing uninitialized: every
// member has a default member initializer or a class type with a
// constexpr default constructor, a union or anonymous union member has
// one variant member with an initializer, the class is empty or has only
// an unnamed bit-field. Value-initialization (`T{}`, `T()`) calls no
// trivial default constructor ([dcl.init]/8.2: zero-initialized), so it is
// constant whatever the constructor. A function whose only default-
// initialization of such a class is on a branch a constant evaluation
// does not take is still a constexpr function ([dcl.constexpr]/5; the
// object is initialized by its constructor, [dcl.init]/7.1, so /3 does
// not apply). clang 22 accepts all; GCC 16.1 rejects that branch and the
// union `U` whose member has an initializer ("uninitialized variable in
// 'constexpr' function"): the text decides. Found running GCC's C++
// testsuite.
//@ args: -std=c++17
//@ strict-diags

struct Init { int a = 0; int b{1}; };
constexpr int init(int i) { Init t; t.a = i; return t.a + t.b; }
static_assert(init(1) == 2, "");

struct Empty {};
struct OnlyBits { int : 3; };
constexpr int empty(int i) { Empty e; OnlyBits o; (void)e; (void)o; return i; }
static_assert(empty(1) == 1, "");

struct M { int m; constexpr M() : m(2) {} };
struct HasM { M m; int arr[2] = {}; };
constexpr int member(int i) { HasM t; t.arr[0] = i; return t.m.m + t.arr[0]; }
static_assert(member(1) == 3, "");

union U { int a = 4; char c; };
struct AnonInit { union { int x = 5; char y; }; };
constexpr int variant(int i) { U u; AnonInit w; return u.a + w.x + i; }
static_assert(variant(1) == 10, "");

struct Base { int b = 6; };
struct Derived : Base {};
constexpr int base(int i) { Derived d; return d.b + i; }
static_assert(base(1) == 7, "");

struct Scalar { int a; };
constexpr int value_init(int i) {
  Scalar s{};
  Scalar t = Scalar();
  Scalar c(s);
  return s.a + t.a + c.a + i;
}
static_assert(value_init(1) == 1, "");

struct User { int a; constexpr User() : a(8) {} };
constexpr int user(int i) { User u; return u.a + i; }
static_assert(user(1) == 9, "");

struct Holder { Scalar s; int n; constexpr Holder() : s(), n(0) {} };
constexpr int holder(int i) { Holder h; return h.s.a + h.n + i; }
static_assert(holder(1) == 1, "");

constexpr int branch(int i) {
  if (i > 5) {
    Scalar t;
    t.a = i;
    return t.a;
  }
  return i;
}
static_assert(branch(1) == 1, "");
