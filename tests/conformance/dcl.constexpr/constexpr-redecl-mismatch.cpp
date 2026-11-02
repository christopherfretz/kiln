// [dcl.constexpr]/1: if any declaration of a function or function template
// has a constexpr or consteval specifier, all its declarations do. Covers
// namespace-scope functions, out-of-line members, constructors, member
// specializations of a class template, and function templates; an explicit
// specialization of a function template is a different entity and may
// differ. Variables are not checked (clang does not either).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:15 error "constexpr declaration of 'f1' follows non-constexpr declaration"
//@ diag: 27:5 note "previous declaration is here"
//@ diag: 30:5 error "non-constexpr declaration of 'f2' follows constexpr declaration"
//@ diag: 29:15 note "previous declaration is here"
//@ diag: 32:15 error "consteval declaration of 'f3' follows non-constexpr declaration"
//@ diag: 31:5 note "previous declaration is here"
//@ diag: 39:8 error "non-constexpr declaration of 'm' follows constexpr declaration"
//@ diag: 34:17 note "previous declaration is here"
//@ diag: 40:18 error "constexpr declaration of 'n' follows non-constexpr declaration"
//@ diag: 35:7 note "previous declaration is here"
//@ diag: 41:4 error "non-constexpr declaration of 'S' follows constexpr declaration"
//@ diag: 36:13 note "previous declaration is here"
//@ diag: 43:34 error "constexpr declaration of 't1' follows non-constexpr declaration"
//@ diag: 42:24 note "previous declaration is here"
//@ diag: 50:36 error "constexpr declaration of 'n' follows non-constexpr declaration"
//@ diag: 47:62 note "previous declaration is here"
//@ diag: 53:14 error "constexpr declaration of 'D' follows non-constexpr declaration"
//@ diag: 52:12 note "previous declaration is here"
int f1();
constexpr int f1() { return 1; }
constexpr int f2();
int f2() { return 1; }
int f3();
consteval int f3() { return 1; }
struct S {
  constexpr int m();
  int n();
  constexpr S();
  static const int a;
};
int S::m() { return 0; }
constexpr int S::n() { return 0; }
S::S() {}
template <class T> int t1();
template <class T> constexpr int t1() { return 0; }
// An explicit specialization may differ from its template.
template <class T> constexpr int g() { return 0; }
template <> int g<int>() { return 1; }
template <class T> struct TT { constexpr int m(); static int n(); };
template <class T> constexpr int TT<T>::m() { return 0; }
template <class T> int TT<T>::n() { return 0; }
template <> constexpr int TT<int>::n() { return 1; }
// An out-of-line constructor.
struct D { D(); };
constexpr D::D() = default;
// Variables are not covered by the rule.
constexpr int S::a = 1;
