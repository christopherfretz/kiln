// [temp.dep.general]/3, [temp.res.general]/4, [temp.arg.type]/1: a member
// class of a class template is a dependent type outside its own definition,
// so a class deriving from it does not find its members by unqualified
// lookup (CWG108), and a type named through it in a template argument needs
// `typename` (a member declaration's `B::X x;` does not, P0634). Ported
// from clang's CXX/drs/cwg1xx.cpp (cwg108) and SemaTemplate/
// typename-specifier.cpp (missing_typename). WP-1084.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:18 error "unknown type name 'X'"
//@ diag: 34:8 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 23:17 note "template parameter is declared here"
//@ diag: 32:10 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 23:17 note "template parameter is declared here"
namespace cwg108 {
template <typename T> struct A {
  struct B { typedef int X; };
  B::X x;
  struct C : B { X x; };
};
}
namespace missing_typename {
template <class T1, class T2> struct pair {};
template <class T>
class ExampleClass1 {
  struct ExampleItem;
  struct ExampleItemSet {
    typedef ExampleItem* iterator;
    ExampleItem* operator[](unsigned);
  };
  void foo() {
    pair<ExampleItemSet::iterator, int> i;
  }
  pair<ExampleItemSet::iterator, int> elt;
};
}
