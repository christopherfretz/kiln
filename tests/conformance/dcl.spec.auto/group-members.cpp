// One placeholder deduction per declarator group, for member and
// instantiated groups too ([dcl.spec.auto.general]/8), and a declarator
// using 'auto' other than for a deduced variable must be alone in its
// group ([dcl.spec.auto.general]/3; clang's err_auto_non_deduced_not_alone).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:20 error "'auto' deduced as 'int' in declaration of 'p' and deduced as 'double' in declaration of 'q'"
//@ diag: 28:20 error "'auto' deduced as 'int' in declaration of 'r' and deduced as 'const int' in declaration of 's'"
//@ diag: 29:20 error "'auto' deduced as 'char' in declaration of 'u' and deduced as 'long' in declaration of 'v'"
//@ diag: 30:8 error "function with deduced return type must be the only declaration in its group"
//@ diag: 31:18 error "function with deduced return type must be the only declaration in its group"
//@ diag: 32:32 error "function with deduced return type must be the only declaration in its group"
//@ diag: 37:12 error "'auto' not allowed in non-static struct member"
//@ diag: 37:12 error "'auto' not allowed in non-static struct member"
//@ diag: 37:17 error "function with deduced return type must be the only declaration in its group"
//@ diag: 39:1 error "'auto' deduced as 'int' in declaration of 'g1' and deduced as 'double' in declaration of 'g2'"
//@ diag: 40:6 error "declaration with trailing return type must be the only declaration in its group"
//@ diag: 42:8 error "declaration with trailing return type must be the only declaration in its group"
//@ diag: 43:22 error "function with deduced return type must be the only declaration in its group"
//@ diag: 49:20 error "'auto' deduced as 'int' in declaration of 'b' and deduced as 'double' in declaration of 'c'"
//@ diag: 48:20 error "'auto' deduced as 'long' in declaration of 'a' and deduced as 'int' in declaration of 'b'"
//@ diag: 56:19 note "in instantiation of member function 'B<long>::g' requested here"
//@ diag: 50:20 error "'auto' deduced as 'long' in declaration of 'a' and deduced as 'int *' in declaration of 'b'"
//@ diag: 57:19 note "in instantiation of member function 'B<long>::k' requested here"
struct A {
  static constexpr auto x = 1, y = 2;           // OK: both int
  static constexpr auto p = 1, q = 2.0;         // int vs double
  static constexpr auto r = 1, *s = &r;         // int vs const int, under *
  static constexpr auto u = 'a', v = 1L, w = 2.0f; // reported once, at v
  auto m1(), m2();                              // [dcl.spec.auto.general]/3
  decltype(auto) d1(), d2();
  static constexpr auto k = 1, f();
};
static_assert(__is_same(decltype(A::x), const int));
static_assert(__is_same(decltype(A::y), const int));

struct N { auto a = 1, b = 2.0; };              // each, then the group

auto g1 = 1, g2 = 2.0;
auto f3() -> int, f4() -> int;
int (*p0)() = nullptr;
auto (*p5)() -> int = p0, q5 = 1;
auto x6 = 1, y6 = 2, f6(), f7();

// Instantiation checks the deductions of the substituted initializers
// ([temp.inst]); a group that failed in the pattern is not checked again.
template<class T> struct B {
  static int g() { auto a = T(1), b = 2; return sizeof(a) + sizeof(b); }
  static int h() { auto a = T(1), b = 2, c = 2.0; return sizeof(a); }
  static int k() { auto *a = (T *)nullptr, b = (int *)nullptr; return 0; }
  // Static data members of a class template are not checked again
  // (clang's member instantiation does not rebuild the group).
  static constexpr auto m = T(1), n = 2;
};
int i1 = B<int>::g();   // OK: int and int
int i2 = B<long>::g();  // long vs int
int i3 = B<long>::k();  // long vs int under '*'
int i4 = B<long>::h();
long i5 = B<long>::m;
