// [class.temporary]/4, -Wdangling with libc++ (WP-674): clang's
// implicit [[gsl::Owner]] std containers (vector, map, optional,
// basic_string, unique_ptr) and [[gsl::Pointer]] views (basic_string_view,
// span) and container iterators, with the tracked members (begin, data,
// c_str, operator[], operator*, value, front, get), diagnose a pointer,
// view or reference into a temporary Owner in a variable, a return and a
// member initializer. std::vector<bool>, an instantiation of a partial
// specialization, is not an Owner. libc++'s std::min/max are
// lifetimebound (libstdc++'s are not: the twin test). Lifetime extension of a
// whole temporary and views of named Owners are fine. clang 19's analysis.
//@ strict-diags
//@ std: c++23
//@ args: -stdlib=libc++
//@ requires: clang<22
//@ diag: 65:27 warning "temporary bound to local reference 'a' will be destroyed at the end of the full-expression"
//@ diag: 65:30 warning "temporary bound to local reference 'a' will be destroyed at the end of the full-expression"
//@ diag: 66:27 warning "temporary bound to local reference 'b' will be destroyed at the end of the full-expression"
//@ diag: 66:30 warning "temporary bound to local reference 'b' will be destroyed at the end of the full-expression"
//@ diag: 67:25 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 68:26 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 70:12 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 71:12 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 72:19 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 73:13 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 74:12 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 75:18 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 76:15 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 77:13 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 81:14 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 89:34 warning "returning address of local temporary object"
//@ diag: 90:49 warning "address of stack memory associated with local variable 's' returned"
//@ diag: 91:42 warning "reference to stack memory associated with parameter 'x' returned"
//@ diag: 91:45 warning "returning reference to local temporary object"
//@ diag: 92:44 warning "address of stack memory associated with local variable 's' returned"
//@ diag: 93:47 warning "address of stack memory associated with parameter 's' returned"
//@ diag: 94:42 warning "reference to stack memory associated with local variable 'v' returned"
//@ diag: 100:40 warning "reference member 'r' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 98:14 note "reference member declared here"
//@ diag: 100:43 warning "reference member 'r' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 98:14 note "reference member declared here"
//@ diag: 101:15 warning "initializing pointer member 'sv' to point to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 97:20 note "pointer member declared here"
//@ diag: 101:42 warning "reference member 'r' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 98:14 note "reference member declared here"
//@ diag: 101:45 warning "reference member 'r' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 98:14 note "reference member declared here"
//@ diag: 101:52 warning "initializing pointer member 'p' to point to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 99:15 note "pointer member declared here"
#include <algorithm>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <vector>
#include <memory>
#include <span>
#include <array>
#include <utility>

std::string make();
std::vector<int> vec();
std::optional<int> opt();

void f() {
  const int &a = std::max(1, 2);
  const int &b = std::min(1, 2);
  std::string_view sv = std::string("x");
  std::string_view sv2 = make();
  const int *p = std::optional<int>(1).operator->();
  int &m = std::map<int, int>()[1];
  int &v = vec()[0];
  const char *c = make().c_str();
  auto it = vec().begin();
  int *d = vec().data();
  const int &o = opt().value();
  int &&o2 = *opt();
  int &fr = vec().front();
  std::span<const int> sp = vec();
  std::string_view sv3;
  sv3 = make();
  int *raw = std::unique_ptr<int>().get();
  const char *c2;
  c2 = make().c_str();
  std::vector<bool> vb;
  auto vbi = std::vector<bool>().begin();
  (void)a; (void)b; (void)sv; (void)sv2; (void)p; (void)m; (void)v; (void)c; (void)it; (void)d; (void)o; (void)o2; (void)fr; (void)sp; (void)raw;
}

std::string_view ret1() { return std::string("x"); }
std::string_view ret2() { std::string s; return s; }
const int &ret3(int x) { return std::max(x, 1); }
const char *ret4() { std::string s; return s.c_str(); }
std::string_view ret5(std::string s) { return s; }
int &ret6() { std::vector<int> v; return v[0]; }

struct M {
  std::string_view sv;
  const int &r;
  const char *p;
  M(std::string s) : sv(s), r(std::max(1, 2)), p(s.c_str()) {}
  M(int) : sv(std::string()), r(std::min(1, 2)), p(make().data()) {}
};

// negatives
void neg() {
  std::string s;
  std::string_view ok = s;
  const int &e = 5;
  const std::string &e2 = make();
  auto &&e3 = vec();
  int x = 1, y = 2;
  const int &mx = std::max(x, y);
  std::string_view ok2 = "lit";
  const char *cp = s.c_str();
  int &vv = e3[0];
  (void)ok; (void)e; (void)e2; (void)mx; (void)ok2; (void)cp; (void)vv;
}
