// [temp.constr.normal]/1.4, [temp.constr.atomic]/1: the type-constraint
// `C<typename T::value_type> U` of a member of A<int> passes an argument
// that fails to substitute, but C's only atom does not use that parameter,
// so the constraint is satisfied for the member template and the member
// class template alike. An argument the atom does use still fails.
// Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-ok
//@ requires: clang>=22
template <class T, class U>
concept C = requires { typename T; };
template <class T, class U>
concept D = requires { typename U; };

template <class T>
struct A {
  template <C<typename T::value_type> U>
  static constexpr int f() { return 1; }
  template <C<typename T::value_type> U>
  struct B { static constexpr int v = 2; };

  template <D<typename T::value_type> U>
  static constexpr int g() { return 3; }
  template <class U>
  static constexpr int g() { return 4; }
};

static_assert(A<int>::f<void>() == 1);
static_assert(A<int>::B<void>::v == 2);
static_assert(A<int>::g<void>() == 4);
