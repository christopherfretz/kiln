// [expr.const]/16-18, [class.base.init], [temp.inst]: an immediate invocation
// in a template definition that is not value-dependent is evaluated at the
// definition, including in a constructor's mem-initializers (a class
// template's, kept unbuilt until instantiation), and again in each
// instantiation that uses it, with the instantiation's context: reported
// when it fails, or making an immediate-escalating instantiation (a
// constexpr function template specialization) immediate. A constexpr
// member of a class template's pattern is not immediate-escalating; its
// failed invocation is reported at the definition (WP-227).
// Pinned to C++23: in C++26 a caught or uncaught throw is evaluated (P3068).
//@ std: c++23
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 89:27 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 89:27 note "in call to 'bad(1)'"
//@ diag: 93:11 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 93:11 note "in call to 'bad(2)'"
//@ diag: 93:30 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 93:30 note "in call to 'bad(3)'"
//@ diag: 93:48 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 93:48 note "in call to 'bad(4)'"
//@ diag: 95:22 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 95:22 note "in call to 'bad(6)'"
//@ diag: 95:38 error "call to consteval function 'ok' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 95:41 note "in call to 'bad(7)'"
//@ diag: 96:27 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 96:27 note "in call to 'bad(8)'"
//@ diag: 97:37 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 97:37 note "in call to 'bad(9)'"
//@ diag: 93:11 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 99:17 note "in instantiation of member function 'S<int>::S' requested here"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 93:11 note "in call to 'bad(2)'"
//@ diag: 93:30 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 93:30 note "in call to 'bad(3)'"
//@ diag: 93:48 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 93:48 note "in call to 'bad(4)'"
//@ diag: 95:22 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 99:17 note "in instantiation of member function 'S<int>::g' requested here"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 95:22 note "in call to 'bad(6)'"
//@ diag: 95:38 error "call to consteval function 'ok' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 95:41 note "in call to 'bad(7)'"
//@ diag: 96:27 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 99:17 note "in instantiation of member function 'S<int>::unused' requested here"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 96:27 note "in call to 'bad(8)'"
//@ diag: 100:42 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 100:42 note "in call to 'bad(10)'"
//@ diag: 104:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 104:17 note "in call to 'bad(11)'"
//@ diag: 105:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 105:17 note "in call to 'bad(12)'"
//@ diag: 104:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 108:15 note "in instantiation of function template specialization 'h<char>' requested here"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 104:17 note "in call to 'bad(11)'"
//@ diag: 105:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 105:17 note "in call to 'bad(12)'"
//@ diag: 110:10 error "call to immediate function 'e<int>' is not a constant expression"
//@ diag: 109:46 note "'e<int>' is an immediate function because its body contains a call to a consteval function 'bad' and that call is not a constant expression"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 109:46 note "in call to 'bad(13)'"
//@ diag: 110:10 note "in call to 'e<int>()'"
//@ diag: 100:42 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 101:8 note "in instantiation of member function 'D<int>::D' requested here"
//@ diag: 85:35 note "subexpression not valid in a constant expression"
//@ diag: 100:42 note "in call to 'bad(10)'"
consteval int bad(int x) { if (x) throw 1; return x; }
consteval int ok(int x) { return x; }
struct B { constexpr B(int) {} };
struct M { int a, b; };
struct N { int a; N() : a(bad(1)) {} };
template<class T> struct S : B {
  int a;
  M m;
  S() : B(bad(2)), a(ok(0) + bad(3)), m{ok(1), bad(4)} {}
  S(int) : B(0), a(ok(5)), m{} {}
  void g() { int z = bad(6); int y = ok(bad(7)); (void)z; (void)y; }
  void unused() { int z = bad(8); (void)z; }
  static constexpr int k() { return bad(9); }
};
template struct S<int>;
template<class T> struct D : B { D() : B(bad(10)) {} };
D<int> d;
S<long> s(1);
template<class T> void h() {
  const int c = bad(11);
  int arr[2] = {bad(12), ok(0)};
  (void)c; (void)arr;
}
template void h<char>();
template<class T> constexpr int e() { return bad(13); }
int ee = e<int>();
