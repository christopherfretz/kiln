//@ expect: compile-fail
//@ std: c++23
//@ diag: 29:3 error "no viable constructor or deduction guide for deduction of template arguments of 'F'"
//@ diag: 31:3 error "no viable constructor or deduction guide for deduction of template arguments of 'C'"
// [over.match.class.deduct]/1 (P2582): a base named by a typedef of a
// simple-template-id contributes the guides of the inherited
// constructors; the inheriting template's parameters that are not
// deduced take their default arguments, and a default that does not
// substitute makes the guide no match. Found running GCC's C++
// testsuite.

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <class T> struct A { A(T) {} };
using B = A<int>;

template <class T = void> struct C : B { using B::B; };
C c = 0;
static_assert(same<decltype(c), C<void>>);

template <class T = char> struct E : A<int> { using A<int>::A; };
E e = 0;
static_assert(same<decltype(e), E<char>>);

template <class T = int, class U = typename T::type> struct F : B {
  using B::B;
};
F f = 0;

C n = nullptr;
