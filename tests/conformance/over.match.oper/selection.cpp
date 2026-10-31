// [over.match.oper]/3: member, non-member (with ADL) and built-in candidates.
//@ expect: compile-ok
// The selected operator function is identified by its return type.
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
struct R1 {}; struct R2 {}; struct R3 {}; struct R4 {};
namespace N {
  struct A { R1 operator+(int) const; R2 operator-() const; };
  R3 operator+(const A &, long);
  R4 operator*(A, A);
  enum E { e };
  R1 operator|(E, E);
}
// [over.match.oper]/3.1-3.2: member and non-member candidates compete.
static_assert(same<decltype(N::A() + 1), R1>::value);
static_assert(same<decltype(N::A() + 1L), R3>::value);
static_assert(same<decltype(-N::A()), R2>::value);
// Non-member candidates are found by ADL.
static_assert(same<decltype(N::A() * N::A()), R4>::value);
static_assert(same<decltype(N::e | N::e), R1>::value);
// [over.match.oper]/3.2: a non-member candidate of an enumeration operand
// must take that enumeration (or a reference to it).
enum F { f0 };
enum G { g0 };
R2 operator&(G, int);
static_assert(same<decltype(f0 & 1), int>::value);
static_assert(same<decltype(g0 & 1), R2>::value);
// [over.match.oper]/3.3: a built-in candidate wins over a worse
// user-declared one.
struct B { operator int() const; };
R3 operator+(B, double);
R4 operator-(B, const char *);
static_assert(same<decltype(B() - 1), int>::value);
static_assert(same<decltype(B() + 1.0), R3>::value);
// Subscript and call operators are members only.
struct S { R1 operator[](int) const; R2 operator[](int, int) const; R3 operator()() const; };
static_assert(same<decltype(S()[0]), R1>::value);
static_assert(same<decltype(S()[0, 1]), R2>::value);
static_assert(same<decltype(S()()), R3>::value);
// Assignment: the implicit copy assignment operator is a candidate.
struct C { C &operator=(int); };
static_assert(same<decltype(C() = C()), C &>::value);
static_assert(same<decltype(C() = 1), C &>::value);
