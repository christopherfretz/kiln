// [temp.expl.spec]/2, /17: explicit specializations of members of
// class template specializations, member templates of specializations
// (`template<> template<>`), and operator template-ids.
//@ expect: compile-ok
template <class T> struct A {
  constexpr int f() const { return 1; }
  template <class U> constexpr int g(U) const { return 2; }
  static constexpr int h() { return 3; }
};
template <> constexpr int A<int>::f() const { return 10; }
template <> template <> constexpr int A<int>::g(char) const { return 20; }
template <> template <> constexpr int A<long>::g<short>(short) const { return 21; }
template <> constexpr int A<char>::h() { return 30; }
struct C { int v; };
template <class T> constexpr bool operator==(C a, T b) { return a.v == b; }
template <> constexpr bool operator==<char>(C, char) { return true; }
struct D {
  template <class T> constexpr int m(T) const { return 0; }
};
template <> constexpr int D::m(int) const { return 9; }
static_assert(A<int>{}.f() == 10 && A<char>{}.f() == 1);
static_assert(A<int>{}.g('c') == 20 && A<int>{}.g(1) == 2 && A<long>{}.g(short()) == 21);
static_assert(A<char>::h() == 30 && A<int>::h() == 3);
static_assert(C{1} == 1 && !(C{1} == 2) && C{1} == 'x');
static_assert(D{}.m(1) == 9 && D{}.m(1.0) == 0);
