// [dcl.typedef]/5-6, [class.name]/2, [basic.scope.scope]/4: in C++ a class
// or enumeration and a typedef-name of the same name in one scope must
// denote the same type. A typedef or alias-declaration after the class that
// names another type is clang's "typedef redefinition with different types"
// (the kind is "typedef" even for an alias-declaration, the previous
// declaration being the class); after a class template or before an alias
// template it is "redefinition as different kind of symbol", reported at the
// alias template's `using`. In namespace, class and block scope, through a
// using-declaration naming the class, after a friend declaration of the
// class (found by redeclaration lookup, as clang does), and with the class still found by name
// afterwards (the invalid typedef is ignored by lookup). The reverse order is
// tag-typedef-same-name-conflict.cpp; valid forms are
// typedef-after-class-same-name-ok.cpp. C keeps tags apart (C17 6.2.3).
// Found auditing WP-1427, fixed by WP-1435.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:14 error "typedef redefinition with different types ('long' vs 't1')"
//@ diag: 46:8 note "previous definition is here"
//@ diag: 50:7 error "cannot initialize a variable of type 'long *' with an rvalue of type 't1 *'"
//@ diag: 52:7 error "typedef redefinition with different types ('int' vs 'u1')"
//@ diag: 51:7 note "previous definition is here"
//@ diag: 54:13 error "typedef redefinition with different types ('int' vs 'e1')"
//@ diag: 53:6 note "previous definition is here"
//@ diag: 56:15 error "typedef redefinition with different types ('short' vs 'e2')"
//@ diag: 55:12 note "previous definition is here"
//@ diag: 60:13 error "typedef redefinition with different types ('f1 *' vs 'f1')"
//@ diag: 59:8 note "previous definition is here"
//@ diag: 62:13 error "redefinition of 'ct' as different kind of symbol"
//@ diag: 61:27 note "previous definition is here"
//@ diag: 64:20 error "redefinition of 'at' as different kind of symbol"
//@ diag: 63:8 note "previous definition is here"
//@ diag: 66:13 error "redefinition of 'at2' as different kind of symbol"
//@ diag: 65:20 note "previous definition is here"
//@ diag: 69:19 error "typedef redefinition with different types ('const s' vs 'n1::s')"
//@ diag: 68:10 note "previous definition is here"
//@ diag: 72:15 error "typedef redefinition with different types ('int' vs 'n1::in::r')"
//@ diag: 70:25 note "previous definition is here"
//@ diag: 75:13 error "typedef redefinition with different types ('int' vs 'fr')"
//@ diag: 74:27 note "previous definition is here"
//@ diag: 78:9 error "typedef redefinition with different types ('int' vs 'outer::m')"
//@ diag: 77:10 note "previous definition is here"
//@ diag: 80:16 error "typedef redefinition with different types ('long' vs 'outer::k')"
//@ diag: 79:8 note "previous definition is here"
//@ diag: 84:15 error "typedef redefinition with different types ('int' vs 'l')"
//@ diag: 83:10 note "previous definition is here"
struct t1 { int a; };
typedef long t1;
struct t1 x1;
t1 y1;
long *py1 = &y1;
union u1 { int a; };
using u1 = int;
enum e1 { E1 };
typedef int e1;
enum class e2 : short { E2 };
typedef short e2;
struct f1;
struct f1 {};
struct f1;
typedef f1 *f1;
template <class T> struct ct {};
typedef int ct;
struct at {};
template <class T> using at = T;
template <class T> using at2 = T;
typedef int at2;
namespace n1 {
  struct s {};
  typedef const s s;
  namespace in { struct r {}; }
  using in::r;
  typedef int r;
}
struct q1 { friend struct fr; };
typedef int fr;
struct outer {
  struct m {};
  using m = int;
  enum k { K };
  typedef long k;
};
void block() {
  struct l { int z; };
  typedef int l;
  l v;
  v.z = 0;
}
