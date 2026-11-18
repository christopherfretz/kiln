// C++98 rules C++11 lifted, under -pedantic-errors: a default member
// initializer, a `;` that declares nothing (in a class, after a member
// function definition, at namespace scope), a trailing comma in an
// enumerator list, `>>` closing two template argument lists, `long long`
// (clang 22 = clang 19; GCC 16.1 rejects each too). Found running GCC's
// C++ testsuite (WP-1645).
//@ expect: compile-fail
//@ std: c++98
//@ args: -pedantic-errors
//@ strict-diags
//@ diag: 28:9 error "default member initializer for non-static data member is a C++11 extension"
//@ diag: 30:3 error "extra ';' inside a struct"
//@ diag: 33:14 error "extra ';' after member function definition"
//@ diag: 35:11 error "extra ';' inside a class"
//@ diag: 36:18 error "extra ';' inside a union"
//@ diag: 37:16 error "commas at the end of enumerator lists are a C++11 extension"
//@ diag: 38:14 error "commas at the end of enumerator lists are a C++11 extension"
//@ diag: 39:1 error "extra ';' outside of a function is a C++11 extension"
//@ diag: 40:15 error "extra ';' outside of a function is a C++11 extension"
//@ diag: 43:8 error "a space is required between consecutive right angle brackets (use '> >')"
//@ diag: 44:1 error "'long long' is a C++11 extension"
//@ diag: 45:10 error "'long long' is a C++11 extension"
//@ diag: 46:1 error "'long long' is a C++11 extension"
//@ diag: 47:1 error "'long long' is a C++11 extension"
//@ diag: 47:17 error "'long long' is a C++11 extension"
//@ diag: 48:18 error "'long long' is a C++11 extension"
struct S {
  int a = 1;
  static const int c = 3;
  ;
  void f() {}
  void g() {};
  void h() {};;
};
class C { ; };
union U { int u; ; };
enum E { E1, E2, };
enum { E3 = 1, };
;
namespace N { ; ; }
template<class T> struct V {};
V<V<int> > ok;
V<V<int>> bad;
long long ll1;
unsigned long long ll2;
long int long ll3;
long long ll6 = 1LL;
int ll7 = sizeof(long long);
int main() { return 0; }
