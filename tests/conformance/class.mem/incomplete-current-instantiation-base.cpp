// [class.mem.general]/8, [class.derived.general]/2: the current
// instantiation named as a base inside its own definition is incomplete.
// Not strict: the note names the class 'B<U>' (clang 19) or 'A::B<U>'
// (clang 22). Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
// (see above: not strict)
//@ diag: 11:55 error "base class has incomplete type"
// (the note's class name differs between clang versions)
template <class T> struct A {
  template <class U> struct B : A<B<U> > { struct C : B<U> {}; };
};
