// [dcl.constexpr]/3 (C++20), [temp.res.general]/6, [expr.prim.lambda.closure]/5:
// before C++23 a constexpr function shall not define a variable of
// non-literal type. In a template the rule is checked at the definition,
// for a variable of non-dependent type only; an instantiation is not
// checked (clang makes it silently non-constexpr). An invalid function
// template stays a candidate whose deduction is a substitution failure, a
// member of a class template is not instantiated. A lambda is checked only
// when it is declared constexpr explicitly (an implicitly constexpr one is
// silently not constexpr). clang 19 and clang 22 agree. WP-414.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:42 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 38:42 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 39:10 error "no matching function for call to 't2'"
//@ diag: 38:33 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 42:43 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 47:25 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 50:19 error "no member named 'm' in 'S<int>'"
//@ diag: 52:52 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 53:14 error "no matching member function for call to 'm'"
//@ diag: 52:44 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 55:30 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 57:34 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 34:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 60:10 error "no matching function for call to object of type '(lambda at "
//@ diag: 57:11 note "candidate template ignored: substitution failure [with auto:1 = int]"
struct B { B(); ~B(); };
// Function templates: a non-dependent variable at the definition; a
// dependent one never (t3<B> is silently not constexpr).
template<class T> constexpr int t1() { B x; return 0; }
template<class T> constexpr int t2() { B x; return 0; }
int u2 = t2<int>();
template<class T> constexpr int t3() { T x; return 0; }
int u3 = t3<B>() + t3<int>();
template<class T> constexpr int t4(T) { B x; return 0; }
int t4(...);
int u4 = t4(1);
// Members of a class template; member templates.
template<class T> struct S {
  constexpr int m() { B x; return 0; }
  constexpr int n() { T x; return 0; }
};
int u5 = S<int>().m();
int u6 = S<B>().n();
struct C { template<class T> constexpr int m() { B x; return 0; } };
int u7 = C().m<int>();
// Lambdas: only one declared constexpr (or consteval) explicitly.
auto l1 = []() constexpr { B x; return 0; };
auto l2 = [] { B x; return 0; };
auto l3 = [](auto) constexpr { B x; return 0; };
auto l4 = [](auto y) constexpr { decltype(y) x; return 0; };
int u8 = l2() + l4(B()) + l4(1);
int u9 = l3(1);
template<class T> constexpr auto tl() { return [] { T x; return 0; }(); }
int u10 = tl<B>();
