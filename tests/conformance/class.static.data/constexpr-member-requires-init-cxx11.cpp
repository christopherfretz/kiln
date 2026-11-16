// C++11 [class.static.data]/3: a static data member declared `constexpr`
// in its class shall have a brace-or-equal-initializer there. Before
// C++17 the in-class declaration is not a definition, so the rule is not
// [dcl.constexpr]'s "must be initialized"; kiln stopped reporting it when
// constexpr static data members stopped being implicitly inline before
// C++17. Found running GCC's C++ testsuite. kiln reports it as "constexpr
// variable declaration must be a definition"; clang's text differs, so
// the message is not pinned.
//@ expect: compile-fail
//@ std: c++11

struct A {
  static constexpr int j = 2;
  static constexpr int k{3};
  static constexpr int i;
};
