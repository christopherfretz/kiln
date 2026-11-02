// [temp.expl.spec]/7: a member class or enumeration of an implicit
// specialization explicitly specialized after its definition was
// instantiated (implicitly, or with its class for an unscoped enumeration)
// or after it was explicitly instantiated; a redefinition; a member class
// only declared specialized is incomplete; [temp.explicit]/3, [dcl.enum]: an
// enumeration is neither explicitly instantiated nor a template; an
// enumerator of the pattern named in a member function is looked up again in
// the specialized enumeration
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:28 error "explicit specialization of 'N' after instantiation"
//@ diag: 42:11 note "implicit instantiation first required here"
//@ diag: 45:34 error "explicit specialization of 'F' after instantiation"
//@ diag: 44:25 note "implicit instantiation first required here"
//@ diag: 47:26 error "explicit specialization of 'U' after instantiation"
//@ diag: 42:1 note "implicit instantiation first required here"
//@ diag: 49:29 error "redefinition of 'N'"
//@ diag: 48:29 note "previous definition is here"
//@ diag: 51:12 error "variable has incomplete type 'X<char>::N'"
//@ diag: 50:29 note "forward declaration of 'X<char>::N'"
//@ diag: 53:33 error "explicit specialization of 'N' after instantiation"
//@ diag: 52:30 note "explicit instantiation first required here"
//@ diag: 54:26 error "enumeration cannot be a template"
//@ diag: 54:13 error "declaration does not declare anything"
//@ diag: 55:10 error "enumerations cannot be explicitly instantiated"
//@ diag: 56:17 error "enumeration cannot be a template"
//@ diag: 56:10 error "declaration does not declare anything"
//@ diag: 58:33 error "redefinition of 'G'"
//@ diag: 57:33 note "previous definition is here"
//@ diag: 40:28 error "no member named 'a' in 'X<long>::F'"
//@ diag: 61:41 note "in instantiation of member function 'X<long>::fa' requested here"
//@ requires: clang<22
// clang 22 emits error: explicit specialization of 'F' after instantiation; WP-355 residual
template <class T>
struct X {
  struct N { int a; };
  enum class F : int { a };
  enum U : int { u };
  enum class G : int;
  int fa() { return int(F::a); }
};
X<int>::N n1;
template <> struct X<int>::N { long z; };
int f1 = (int)X<short>::F::a;
template <> enum class X<short>::F : int { b };
template <> enum class X<int>::F : int { c };
template <> enum X<int>::U : int { v };
template <> struct X<long>::N { long z; };
template <> struct X<long>::N { long z; };
template <> struct X<char>::N;
X<char>::N n2;
template struct X<unsigned>::N;
template <> struct X<unsigned>::N { };
template <> enum H : int { h };
template enum class X<int>::G;
template enum H2;
template <> enum class X<long>::G : int { g };
template <> enum class X<long>::G : int { g2 };
template <> enum class X<long>::F : int;
template <> enum class X<long>::F : int { k };
int f2 = (int)X<long>::F::k + X<long>().fa();
