// [temp.mem]/2: a friend class template declared in a local class is a
// member template of it too: clang's "templates cannot be declared
// inside of a local class" at `template` (WP-1041).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "templates cannot be declared inside of a local class"
//@ diag: 13:5 error "templates cannot be declared inside of a local class"
//@ diag: 16:16 error "templates cannot be declared inside of a local class"
template<class T> struct X {};
void f() {
  struct L {
    template<class T> friend class Y;
    template<class T> friend struct X;
  };
  struct M {
    struct N { template<int I> friend class Z; };
  };
}
