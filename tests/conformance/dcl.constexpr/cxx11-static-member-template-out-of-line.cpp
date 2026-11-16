// C++11 [dcl.constexpr]/8 made a constexpr non-static member function
// const; a static one is not, and its out-of-line definition, which
// cannot repeat `static`, is not either. That held for member functions
// but not for member function templates. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ std: c++11

struct A {
  template <class T> static constexpr T g(T);
  template <class> static constexpr bool h();
};
template <class U> constexpr U A::g(U u) { return u; }
template <class> constexpr bool A::h() { return true; }
static_assert(A::g(3) == 3 && A::h<int>(), "");
