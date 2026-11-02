// [class.friend]/6: a function can be defined in a friend declaration only of a
// non-local class. A local class's friend definition that validly redeclares a
// block-scope function is "friend function cannot be defined in a local class"
// (also in a function template, at the definition, once); an invalid one
// (nothing to redeclare, a mismatched exception specification) gets only its
// own error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:17 error "friend function cannot be defined in a local class"
//@ diag: 21:17 error "'h' is missing exception specification 'noexcept'"
//@ diag: 17:8 note "previous declaration is here"
//@ diag: 22:17 error "no matching function found in local scope"
//@ diag: 25:19 error "friend function cannot be defined in a local class"
//@ diag: 32:17 error "friend function cannot be defined in a local class"
void f() {
  void g();
  void h(int) noexcept;
  void k();
  struct L {
    friend void g() {}
    friend void h(int) {}
    friend void q() {}
    friend void k();
    struct N {
      friend void k() {}
    };
  };
}
template <class T> void t() {
  void g(T);
  struct L {
    friend void g(T) {}
  };
}
template void t<int>();
