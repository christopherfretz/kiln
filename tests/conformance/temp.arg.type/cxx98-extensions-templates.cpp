// C++98 rules C++11 (C++14) lifted, under -pedantic-errors: a friend type
// without a class-key, a default template argument of a function template,
// a variable template, a local or unnamed type as a template argument
// (clang 22 = clang 19; GCC 16.1 rejects each too). Found running GCC's
// C++ testsuite (WP-1645).
//@ expect: compile-fail
//@ std: c++98
//@ args: -pedantic-errors
//@ strict-diags
//@ diag: 29:3 error "unelaborated friend declaration is a C++11 extension; specify 'struct' to befriend 'Y'"
//@ diag: 30:3 error "unelaborated friend declaration is a C++11 extension; specify 'struct' to befriend 'YT' (aka 'Y')"
//@ diag: 31:3 error "non-class friend type 'int' is a C++11 extension"
//@ diag: 32:3 error "non-class friend type 'T' is a C++11 extension"
//@ diag: 35:16 error "default template arguments for a function template are a C++11 extension"
//@ diag: 36:23 error "default template arguments for a function template are a C++11 extension"
//@ diag: 38:18 error "default template arguments for a function template are a C++11 extension"
//@ diag: 39:30 error "variable templates are a C++14 extension"
//@ diag: 41:24 error "variable templates are a C++14 extension"
//@ diag: 42:21 error "variable templates are a C++14 extension"
//@ diag: 49:5 error "template argument uses local type 'L'"
//@ diag: 50:5 error "template argument uses local type 'L'"
//@ diag: 51:5 error "template argument uses local type 'LE'"
//@ diag: 53:5 error "template argument uses unnamed type"
//@ diag: 45:1 note "unnamed type used in template argument was declared here"
struct Y {};
typedef Y YT;
template<class T> struct V {};
template<class T> struct Fr {
  friend Y;
  friend YT;
  friend int;
  friend T;
  friend class V<T>;
};
template<class T = int> void f1() {}
template<class T, int N = 2> void f2(T) {}
struct M {
  template<class T = int> void m() {}
  template<class T> static T sv;
};
template<class T> T M::sv = T();
template<class T> T vt = T();
template<> int vt<int> = 1;
typedef struct { int x; } Named;
struct { int y; } anon_obj;
void g() {
  struct L {};
  enum LE { A };
  V<L> v1;
  V<L *> v2;
  V<LE> v3;
  V<Named> ok;
  V<__typeof__(anon_obj) *> v4;
  (void)v1; (void)v2; (void)v3; (void)ok; (void)v4;
}
int main() { return 0; }
