// [dcl.meaning.general]/2: a class or enumeration declared in a
// member-specification has an unqualified name. A qualifier naming another
// scope is "non-friend class member cannot have a qualified name", one naming
// the class itself is "extra qualification"; both at the name. A qualified
// forward declaration is also an error of its own.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 18:13 error "non-friend class member 'x' cannot have a qualified name"
//@ diag: 20:13 error "extra qualification on member 'inner'"
//@ diag: 21:11 error "non-friend class member 'e' cannot have a qualified name"
//@ diag: 23:17 error "extra qualification on member 'ie'"
//@ diag: 24:31 error "non-friend class member 't' cannot have a qualified name"
//@ diag: 25:10 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 25:13 error "non-friend class member 'z' cannot have a qualified name"
//@ diag: 27:14 error "non-friend class member 'J' cannot have a qualified name"
namespace N { struct x; enum e : int; template<class T> struct t; struct z; }
struct y {
  struct N::x { int a; };
  struct inner;
  struct y::inner { };
  enum N::e : int { A };
  enum class ie;
  enum class y::ie { B };
  template<class T> struct N::t { };
  struct N::z;
  struct I2 { struct J; };
  struct I2::J {};
};
