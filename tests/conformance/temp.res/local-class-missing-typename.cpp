// [temp.pre]/8, [temp.dep.type]/1, [temp.res.general]/4: a class declared in a
// dependent context (a local class of a function template, of a generic
// lambda or of a member function of a local class, a member class of a class
// template) is a dependent type, and the current instantiation only inside
// its own definition. A type named through it anywhere else is a dependent
// qualified name, which an expression takes for a value: written without
// `typename` it is diagnosed when the template is instantiated, at the
// qualifier, as for `T::N()` (clang's "missing 'typename'"). Inside the
// class itself (`L::N()` in `L::get`) none is needed. WP-1084, WP-904.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:3 error "missing 'typename' prior to dependent type name 'L::N'"
//@ diag: 69:3 note "in instantiation of function template specialization 'local_class<int>' requested here"
//@ diag: 35:16 error "missing 'typename' prior to dependent type name 'L::N'"
//@ diag: 39:3 error "missing 'typename' prior to dependent type name 'O::In::In2'"
//@ diag: 70:3 note "in instantiation of function template specialization 'nested_qualifier<int>' requested here"
//@ diag: 44:19 error "missing 'typename' prior to dependent type name 'L::N'"
//@ diag: 46:3 note "in instantiation of function template specialization 'generic_lambda()::"
//@ diag: 71:3 note "in instantiation of function template specialization 'generic_lambda<int>' requested here"
//@ diag: 51:29 error "missing 'typename' prior to dependent type name 'Inner::N'"
//@ diag: 49:10 note "in instantiation of member function 'nested_local()::Outer::m' requested here"
//@ diag: 72:3 note "in instantiation of function template specialization 'nested_local<int>' requested here"
//@ diag: 52:45 error "missing 'typename' prior to dependent type name 'Q::N'"
//@ diag: 49:10 note "in instantiation of member function 'nested_local()::Outer::own' requested here"
//@ diag: 72:3 note "in instantiation of function template specialization 'nested_local<int>' requested here"
//@ diag: 59:14 error "missing 'typename' prior to dependent type name 'A::N'"
//@ diag: 73:12 note "in instantiation of member function 'X<int>::m' requested here"
//@ diag: 63:18 error "missing 'typename' prior to dependent type name 'L::N'"
//@ diag: 74:12 note "in instantiation of member function 'X<int>::k' requested here"
//@ diag: 66:45 error "missing 'typename' prior to dependent type name 'S::N'"
//@ diag: 75:3 note "in instantiation of function template specialization 'dependent_param<S>' requested here"
template <class T> void local_class() {
  struct L { struct N {}; };
  L::N();
  (void)sizeof(L::N);
}
template <class T> void nested_qualifier() {
  struct O { struct In { struct In2 {}; }; };
  O::In::In2();
}
template <class T> void generic_lambda() {
  auto g = [](auto) {
    struct L { struct N {}; };
    return sizeof(L::N);
  };
  g(0);
}
template <class T> void nested_local() {
  struct Outer {
    struct Inner { struct N {}; };
    void m() { (void)sizeof(Inner::N); }
    void own() { struct Q { struct N {}; }; Q::N(); }
  };
  Outer().m();
  Outer().own();
}
template <class T> struct X {
  struct A { struct N {}; };
  void m() { A::N(); }
  void k() {
    struct L { struct N {}; int get() { L::N(); N(); return 0; } };
    L().get();
    (void)sizeof(L::N);
  }
};
template <class T> void dependent_param() { T::N(); }
struct S { struct N {}; };
void use() {
  local_class<int>();
  nested_qualifier<int>();
  generic_lambda<int>();
  nested_local<int>();
  X<int>().m();
  X<int>().k();
  dependent_param<S>();
}
