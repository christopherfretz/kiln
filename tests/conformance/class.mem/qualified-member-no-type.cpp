// [class.mem.general]/1, [dcl.meaning.general]/2: a member declaration starting
// with a qualified name that is not a type ('R::f(v);', 'R::f<int> a;') recovers as
// at namespace scope: 'no type named' / 'no template named' in the namespace, or a
// qualified member name with no type-specifier; later members are unaffected (WP-421).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:6 error "no type named 'f' in namespace 'R'"
//@ diag: 15:6 error "non-friend class member 'h' cannot have a qualified name"
//@ diag: 15:6 error "a type specifier is required for all declarations"
//@ diag: 16:6 error "no template named 'f' in namespace 'R'"
//@ diag: 17:6 error "no type named 'x' in namespace 'R'"
namespace R { int f(int); int v; }
struct T {
  R::f(v);
  R::h(int);
  R::f<int> a;
  R::x b;
  int ok;
};
T t;
int after = t.ok;
