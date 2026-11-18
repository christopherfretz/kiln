// [dcl.constexpr]/6 and /10, [expr.const], [basic.life], [class.dtor]: a
// constexpr local of a constexpr function holds the value of its
// initializer, a constant expression evaluated at its point (kiln
// materializes the value it recorded there instead of evaluating the
// initializer at each call; spec/oven.md "Constexpr locals", WP-1673).
// Every call still has its own object: a class value whose member
// function is called, an array, a union with its active member, pointers
// to a static object, a string literal, a function and one past the end,
// a member pointer, a class with a mutable member that is modified, a
// constexpr destructor run (and its writes) per call, distinct addresses
// across locals and across recursive calls, a local in a template
// specialization and in a lambda body.
//@ expect: compile-ok
//@ strict-diags
struct P {
  int a;
  double b;
  constexpr int sum() const { return a + static_cast<int>(b); }
};
union U { int i; float f; };
struct M { int m; };
struct MU { mutable int m; int n; };
struct D {
  int v;
  constexpr ~D() { v = 0; }  // a const object is writable while it is destroyed
};
int g;
const int cg = 7;
constexpr int f1() { return 3; }

constexpr int arr() {
  constexpr int a[4] = {1, 2, 3, 4};
  int s = 0;
  for (int x : a) s += x;
  return s;
}
static_assert(arr() == 10);

constexpr int cls() { constexpr P p{2, 3.5}; return p.sum(); }
static_assert(cls() == 5);

constexpr bool un() { constexpr U u{.f = 1.0f}; return u.f == 1.0f; }
static_assert(un());

constexpr bool ptrs() {
  constexpr int *pg = &g;
  constexpr const int *pc = &cg;
  constexpr int (*pf)() = f1;
  constexpr int M::*pm = &M::m;
  constexpr const char *s = "xy";
  constexpr const int *end = &cg + 1;
  return pg == &g && *pc == 7 && pf() == 3 && pf == &f1 && pm == &M::m && s[1] == 'y' &&
         end == &cg + 1 && end - 1 == pc;
}
static_assert(ptrs());

constexpr int mut() {
  constexpr MU mu{1, 2};
  mu.m += 5;
  return mu.m + mu.n;
}
static_assert(mut() == 8);
static_assert(mut() + mut() == 16);

constexpr int dtor() { constexpr D d{4}; return d.v; }
constexpr int dtor_twice() { return dtor() + dtor(); }
static_assert(dtor_twice() == 8);

constexpr bool two_locals() { constexpr int a = 1; constexpr int b = 1; return &a != &b && a == b; }
static_assert(two_locals());

constexpr bool recursive(int n, const int *prev) {
  constexpr int a = 1;
  if (prev && prev == &a) return false;
  return n == 0 || recursive(n - 1, &a);
}
static_assert(recursive(3, nullptr));

template <class T> constexpr int tl() { constexpr T x = sizeof(T); return x; }
static_assert(tl<int>() == sizeof(int) && tl<char>() == 1);

constexpr int lam() {
  auto l = [](auto y) { constexpr int k = 9; return k + y; };
  return l(1) + l(2L);
}
static_assert(lam() == 21);

constexpr int loop(int n) {
  int t = 0;
  for (int i = 0; i < n; ++i) {
    constexpr P p{1, 1.0};
    t += p.sum();
  }
  return t;
}
static_assert(loop(100) == 200);

int main() {}
