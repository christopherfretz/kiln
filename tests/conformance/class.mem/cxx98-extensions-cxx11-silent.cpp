// The C++98 extension diagnostics of WP-1645 stay silent from C++11 on,
// under -pedantic-errors too (clang 22, clang 19).
//@ expect: compile-ok
//@ std: c++11
//@ args: -pedantic-errors
//@ strict-diags
struct S {
  int a = 1;
  static const int c = 3;
  void g() {};
};
enum E { E1, E2, };
;
namespace N { ; ; }
template<class T> struct V {};
V<V<int>> two;
long long ll1 = 1LL;
unsigned long long ll2;
struct Y {};
template<class T> struct Fr {
  friend Y;
  friend int;
  friend T;
};
template<class T = int> void f1() {}
void g() {
  struct L {};
  V<L> v1;
  V<L *> v2;
  (void)v1; (void)v2;
}
int main() { return 0; }
