// [temp.expl.spec]/3, [dcl.meaning.general]/2: an explicit specialization
// of a member of an implicit class template specialization is declared in
// the class or in a namespace enclosing it. clang's
// CheckMemberSpecialization -> CheckTemplateSpecializationScope diagnoses
// a member class or enumeration (scoped or unscoped, opaque or defined,
// declaration or definition) against the class pattern 'A'; a member
// function or static data member is diagnosed earlier by
// diagnoseQualifiedDeclaration against 'A<int>' (WP-1178). An inline
// namespace of N and a namespace nested in N do not enclose A; in a class
// the qualifier is an error too. At namespace scope the specialization is
// still declared (clang's recovery): the later uses are fine.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 59:31 error "member class specialization of 'I' not in class 'A' or an enclosing namespace"
//@ diag: 43:10 note "explicitly specialized declaration is here"
//@ diag: 60:31 error "member class specialization of 'J' not in class 'A' or an enclosing namespace"
//@ diag: 44:10 note "explicitly specialized declaration is here"
//@ diag: 61:35 error "member enumeration specialization of 'E' not in class 'A' or an enclosing namespace"
//@ diag: 45:14 note "explicitly specialized declaration is here"
//@ diag: 62:35 error "member enumeration specialization of 'F' not in class 'A' or an enclosing namespace"
//@ diag: 46:14 note "explicitly specialized declaration is here"
//@ diag: 63:29 error "member enumeration specialization of 'U' not in class 'A' or an enclosing namespace"
//@ diag: 47:8 note "explicitly specialized declaration is here"
//@ diag: 64:29 error "member enumeration specialization of 'V' not in class 'A' or an enclosing namespace"
//@ diag: 48:8 note "explicitly specialized declaration is here"
//@ diag: 65:29 error "cannot define or redeclare 'f' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 66:29 error "cannot define or redeclare 'f2' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 67:28 error "cannot define or redeclare 'v' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 68:28 error "cannot define or redeclare 'v2' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 75:29 error "member class specialization of 'I' not in class 'A' or an enclosing namespace"
//@ diag: 43:10 note "explicitly specialized declaration is here"
//@ diag: 76:27 error "cannot define or redeclare 'f' here because namespace 'L' does not enclose namespace 'A<char>'"
//@ diag: 79:32 error "member class specialization of 'J' not in class 'A' or an enclosing namespace"
//@ diag: 44:10 note "explicitly specialized declaration is here"
//@ diag: 80:30 error "member enumeration specialization of 'U' not in class 'A' or an enclosing namespace"
//@ diag: 47:8 note "explicitly specialized declaration is here"
//@ diag: 86:35 error "non-friend class member 'I' cannot have a qualified name"
//@ diag: 86:35 error "member class specialization of 'I' not in class 'A' or an enclosing namespace"
//@ diag: 43:10 note "explicitly specialized declaration is here"
namespace N {
template <class T> struct A {
  struct I;
  struct J;
  enum class E;
  enum class F : int;
  enum U : int;
  enum V : int;
  void f();
  void f2();
  static int v;
  static int v2;
};
inline namespace L {}
namespace In {}
} // namespace N

namespace M {
template <> struct N::A<int>::I {};
template <> struct N::A<int>::J;
template <> enum class N::A<int>::E { x };
template <> enum class N::A<int>::F : int;
template <> enum N::A<int>::U : int { y };
template <> enum N::A<int>::V : int;
template <> void N::A<int>::f() {}
template <> void N::A<int>::f2();
template <> int N::A<int>::v = 1;
template <> int N::A<int>::v2;
} // namespace M

// An inline namespace of N, or a namespace nested in it, does not
// enclose A.
namespace N {
inline namespace L {
template <> struct A<char>::I {};
template <> void A<char>::f() {}
} // namespace L
namespace In {
template <> struct N::A<char>::J;
template <> enum N::A<char>::U : int;
} // namespace In
} // namespace N

// In a class: the qualifier is diagnosed, then the scope.
struct S {
  template <> struct N::A<short>::I {};
};

// Recovery: at namespace scope the member is still specialized, with
// the definition given.
int use = sizeof(N::A<int>::I) + sizeof(N::A<char>::I);
auto e = N::A<int>::E::x;
auto y = N::A<int>::y;
