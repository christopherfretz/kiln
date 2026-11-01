// [namespace.udecl]/10, [class.friend]/7: an unqualified friend function or function template
// that corresponds to the target of a using-declarator of its namespace would redeclare that
// target: "cannot befriend target of using declaration" (clang), also when the class is a
// template and the friend does not depend on its parameters (WP-273).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "cannot befriend target of using declaration"
//@ diag: 18:6 note "target of using declaration"
//@ diag: 22:11 note "using declaration"
//@ diag: 26:34 error "cannot befriend target of using declaration"
//@ diag: 19:25 note "target of using declaration"
//@ diag: 22:27 note "using declaration"
//@ diag: 28:44 error "cannot befriend target of using declaration"
//@ diag: 18:6 note "target of using declaration"
//@ diag: 22:11 note "using declaration"
namespace ep {
struct S2;
void swap(S2&, S2&);
template <class T> void tsw(T&);
}
namespace std {
using ep::swap; using ep::tsw;
struct S {
  friend void swap(S&, S&) {}
  friend void swap(ep::S2&, ep::S2&);
  template <class T> friend void tsw(T&);
};
template <class T> struct TS { friend void swap(ep::S2&, ep::S2&); };
TS<int> x;
}
