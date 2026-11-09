// [namespace.udecl]/11: functions with different parameter-type-lists
// overload with using-declared ones whatever their return types, the
// same entity redeclared is no conflict, two using-declarators may
// introduce corresponding functions, and kiln (as clang and gcc) does
// not check a block scope's declarations again at its instantiation
// (clang's namespace.udecl/p11.cpp test5; WP-1042).
//@ expect: compile-ok
//@ strict-diags
namespace ns { void foo(); void bar(int); }
namespace ns2 { int foo(); }
namespace a {
  int foo(int);
  using ns::foo;
  long foo(long);
}
namespace b {
  void foo();
  using b::foo;
  void foo() {}
}
namespace c {
  using ns::foo;
  using ns2::foo;
}
template <class T> struct C {
  void g() {
    int bar(T);
    using ns::bar;
  }
};
template struct C<int>;
