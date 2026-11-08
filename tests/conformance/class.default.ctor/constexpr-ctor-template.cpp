// [class.default.ctor]/1, /4: a default constructor is any constructor
// callable without arguments, a constructor template included (no
// parameters, or a first parameter with a default argument or that is a
// pack). A constexpr one makes a member's default initialization
// constant, so the enclosing class's implicit default constructor is
// constexpr (clang addedMember / hasConstexprDefaultConstructor), as for
// a member like libc++'s std::tuple. C++20: from C++23 the implicit
// default constructor is constexpr regardless ([dcl.constexpr]/3, P2448).
//@ std: c++20
//@ strict-diags
struct F { int x = 0; };
template <class T> struct Tup {
  T v;
  template <class U = T> constexpr Tup() : v() {}
};
struct H { Tup<F> t; constexpr int p() const { return 1; } };
consteval int g() { return H().p(); }
static_assert(g() == 1);

struct A { template <class U = int> constexpr A() {} };
struct B { A a; };
static_assert((B(), true));

struct P { template <class... Ts> constexpr P(Ts...) {} };
struct Q { P p; };
static_assert((Q(), true));

struct D { template <class T = int> constexpr D(T = T()) {} D(int, int) {} };
struct E { D d; int i = 2; };
constexpr E e{};
static_assert(e.i == 2);
