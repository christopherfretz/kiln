// [namespace.def.general]/7, [dcl.type.elab]/1-2, [basic.lookup.general]/1: an inline namespace's
// members are found by lookup in the enclosing namespace, so a tag declared in both is ambiguous
// at an elaborated reference or template-id; `struct X;` declares a new tag of the current scope
// only; a standalone class or opaque enum declaration cannot be qualified (clang's
// `err_standalone_class_nested_name_specifier`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:10 error "reference to 'X' is ambiguous"
//@ diag: 26:10 note "candidate found by name lookup is 'N::X'"
//@ diag: 27:31 note "candidate found by name lookup is 'N::I::X'"
//@ diag: 34:10 error "reference to 'X' is ambiguous"
//@ diag: 33:10 note "candidate found by name lookup is 'Q::X'"
//@ diag: 31:31 note "candidate found by name lookup is 'Q::I::X'"
//@ diag: 40:8 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 41:7 error "forward declaration of union cannot have a nested name specifier"
//@ diag: 42:6 error "forward declaration of enum cannot have a nested name specifier"
//@ diag: 43:12 error "forward declaration of enum cannot have a nested name specifier"
//@ diag: 44:8 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 46:8 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 46:11 error "no struct named 'W' in namespace 'R'"
//@ diag: 49:27 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 54:3 error "reference to 'X' is ambiguous"
//@ diag: 53:27 note "candidate found by name lookup is 'T::X'"
//@ diag: 52:48 note "candidate found by name lookup is 'T::I::X'"
namespace N {
  struct X;
  inline namespace I { struct X; }
  struct X *p2;
}
namespace Q {
  inline namespace I { struct X; }
  struct X *r;
  struct X;
  struct X *r2;
}
namespace R {
  inline namespace I { struct X {}; union U; enum E : int; }
  enum class F;
}
struct R::X;
union R::U;
enum R::E : int;
enum class R::F;
struct ::R::X;
R::X rx;
struct R::W;
template <class T> struct A { struct B; };
template <> struct A<int>::B;
template <class T> struct A<T>::B;
template struct A<long>;
namespace T {
  inline namespace I { template <class> struct X; }
  template <class> struct X;
  X<int> *t;
}
namespace S {
  inline namespace I { struct X; }
  struct X *s;
}
struct S::X { int a; };
S::X sx;
