// [class.mem.general]/1, [class.abstract]/2: a pure-specifier belongs to
// a member-declarator, and a friend declaration declares no member of
// its class, so `= 0` on a friend function is ill-formed: clang's
// "friend declaration cannot have a pure-specifier" at the name, for a
// friend member of another class, a namespace-scope friend, a friend
// template, and in a class template. The friend is still declared, so
// only the redeclared lookup rules apply after it (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:18 error "friend declaration cannot have a pure-specifier"
//@ diag: 20:15 error "friend declaration cannot have a pure-specifier"
//@ diag: 21:18 error "friend declaration cannot have a pure-specifier"
//@ diag: 22:33 error "friend declaration cannot have a pure-specifier"
//@ diag: 24:42 error "friend declaration cannot have a pure-specifier"
//@ diag: 26:14 error "use of undeclared identifier 'k'"
struct A { void g(); virtual void v(); };
template<class T> void tf(T);
struct B {
  friend void A::g() = 0;
  friend void k() = 0;
  friend void A::v() = 0;
  template<class T> friend void tf(T) = 0;
};
template<class T> struct C { friend void n(T) = 0; };
C<int> c;
void use() { k(); }
