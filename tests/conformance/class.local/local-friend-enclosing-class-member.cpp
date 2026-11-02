// [class.friend]/11, [class.local]: the redeclaration lookup of a local class's
// unqualified friend searches the class and the local classes enclosing it, then
// the innermost enclosing block. The first class declaring the name decides: a
// corresponding member function is befriended, a data member is "redefinition as
// different kind of symbol", any other overload set is "no matching function
// found in local scope". Members of base classes and members declared later are
// not found.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:19 error "no matching function found in local scope"
//@ diag: 30:19 error "redefinition of 'data' as different kind of symbol"
//@ diag: 24:9 note "previous definition is here"
//@ diag: 31:19 error "no matching function found in local scope"
//@ diag: 33:19 error "no matching function found in local scope"
//@ diag: 47:19 error "no matching function found in local scope"
struct B {
  void inherited();
};
void f() {
  void g(int);
  struct L : B {
    void m();
    static void s(int);
    int data;
    friend void m();
    struct N {
      friend void m();
      friend void m(int);
      friend void s(int);
      friend void data();
      friend void inherited();
      friend void g(int);
      friend void later();
      struct O {
        friend void m();
        friend void g(int);
      };
    };
    void later();
  };
}
template <class T> void t() {
  struct L {
    void m(T);
    struct N {
      friend void m(T);
      friend void m(int);
    };
  };
}
template void t<int>();
template void t<long>();
