// [class.union.anon], and the GNU/MS anonymous struct: anonymous members of
// a class template, nested, used in the template, through `this`, after
// instantiation, in constant evaluation and in aggregate initialization,
// and a `static` anonymous union in a function template (GCC 16.1, clang 22
// with -Wno-gnu-anonymous-struct). Found running GCC's C++ testsuite
// (WP-1466).
//@ expect: run
//@ std: c++20
//@ strict-diags
template<class T> struct S {
  union { T a; int b; };
  struct { T c; union { T d; long e; }; };
  constexpr T get() const { return a + c + d; }
  T viaThis() { return this->a + c; }
};
template<class T> int g() { static union { T x; int y; }; x = 1; return y; }
constexpr int ce() { S<int> s{}; s.a = 1; s.c = 2; s.d = 3; return s.get(); }
static_assert(ce() == 6);
int main() {
  S<int> s; s.a = 1; s.c = 2; s.d = 3;
  S<int> s2 = { {3}, {4, {5}} };
  int r = s.viaThis() + s2.get() + g<int>();
  return r == 3 + 12 + 1 ? 0 : 1;
}
