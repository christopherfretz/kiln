// [class.access]/3: members of a class defined with the class-key `class`
// are private by default, and public with `struct` or `union`; the default
// base access follows the class-key too ([class.access.base]/2). For a class
// template it is the class-key of the definition that is instantiated, not
// of the declaration the specialization is first named through (libstdc++ 16
// <format>: `template<typename _CharT> class _Scanner;` then
// `struct _Scanner { ... }`, befriended by `basic_format_parse_context`
// before its definition; WP-525).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:28 error "'y' is a private member of 'C<int>'"
//@ diag: 33:7 note "implicitly declared private here"
//@ diag: 37:34 error "'a' is a private member of 'A'"
//@ diag: 32:30 note "constrained by implicitly private inheritance here"
//@ diag: 16:16 note "member is declared here"
struct A { int a; };
template <class T> class S;
template <class T> struct P { friend S<T>; };
P<char> pc;
template <class T> struct S : A {
  using it = int;
  S(int) {}
  int x;
};
struct D : S<int> {
  D() : S<int>(1) {}
  S<int>::it z;
};
int f(S<char> s) { return s.x + s.a; }

template <class T> struct C;
template <class T> class C : A {
  int y;
public:
  C() {}
};
int g(C<int> c) { return c.y + c.a; }
