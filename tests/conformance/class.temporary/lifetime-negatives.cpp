// [class.temporary]/6, lifetime analysis negatives (WP-674): lifetime
// extension through a reference to a whole temporary (at namespace and
// block scope, and of an aggregate's reference member), lifetimebound
// calls on named, parameter and static-storage arguments, views of named
// and parameter Owners, Pointers from Pointers (std::string_view temporaries
// own nothing), untracked members (unique_ptr::release), a range-for over a
// temporary, and returns of parameters through lifetimebound are not
// diagnosed. No warning here is clang 19's.
//@ strict-diags
//@ std: c++23
//@ requires: clang<22
#include <memory>
#include <string>
#include <string_view>
#include <vector>

const int &pick(const int &a [[clang::lifetimebound]], const int &b);
struct S {
  int v;
  const int &get() const [[clang::lifetimebound]];
};
struct A { const int &r; };
struct [[gsl::Owner(int)]] Own { Own(); ~Own(); int *begin(); };
struct [[gsl::Pointer(int)]] Ptr { Ptr(); Ptr(const Own &); Ptr(const Ptr &); };
std::string make();
std::vector<int> vec();
Own own();

const int &ext = 5;
const S &es = S{1};
const int &eg = es.get();
A ea{1};

void f(const int &param, const std::string &ps) {
  const std::string &s = make();
  std::string_view sv = s;
  auto &&v = vec();
  int &x = v[0];
  std::string_view lit = "lit";
  int y = 1;
  const int &py = pick(y, 2);
  const int &pp = pick(param, 3);
  const S &ls = S{2};
  const int &lg = ls.get();
  A a{1};
  A aa = {2};
  const char *vd = std::string_view("x").data();
  std::string_view vv = std::string_view(s);
  int *rel = std::unique_ptr<int>(new int(1)).release();
  int sum = 0;
  for (int i : vec()) sum += i;
  Own o;
  Ptr p = o;
  Ptr p2 = Ptr(o);
  Ptr p3;
  p3 = o;
  int *ob = o.begin();
  std::string_view psv = ps;
  (void)sv; (void)x; (void)lit; (void)py; (void)pp; (void)lg; (void)a; (void)aa;
  (void)vd; (void)vv; (void)rel; (void)sum; (void)p; (void)p2; (void)ob; (void)psv;
  delete rel;
}

std::string_view r1(const std::string &s [[clang::lifetimebound]]) { return s; }
const int &r2(const int &a) { return pick(a, 1); }
Ptr r3(const Own &o) { return o; }
std::string_view r4(std::string_view s) { return s; }
const int &r5() { static int z = 0; return pick(z, 1); }

struct M {
  const int &r;
  std::string_view sv;
  Ptr p;
  M(const int &a, const std::string &s, const Own &o) : r(pick(a, 1)), sv(s), p(o) {}
};
