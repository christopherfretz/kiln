// [class.access]/4: access control applies to type names as to other names:
// a qualified type name, each nested-name-specifier component, a member
// template name, a typename-specifier and an elaborated-type-specifier.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:4 error "'Priv' is a private member of 'A'"
//@ diag: 27:10 note "implicitly declared private here"
//@ diag: 35:4 error "'Priv' is a private member of 'A'"
//@ diag: 27:10 note "implicitly declared private here"
//@ diag: 36:4 error "'I' is a private member of 'A'"
//@ diag: 28:15 note "implicitly declared private here"
//@ diag: 37:4 error "'E' is a private member of 'A'"
//@ diag: 29:8 note "implicitly declared private here"
//@ diag: 38:4 error "'PT' is a private member of 'A'"
//@ diag: 30:26 note "implicitly declared private here"
//@ diag: 39:13 error "'Priv' is a private member of 'A'"
//@ diag: 27:10 note "implicitly declared private here"
//@ diag: 40:9 error "'E' is a private member of 'A'"
//@ diag: 29:8 note "implicitly declared private here"
//@ diag: 41:15 error "'I' is a private member of 'A'"
//@ diag: 28:15 note "implicitly declared private here"
//@ diag: 45:13 error "'Priv' is a private member of 'A'"
//@ diag: 27:10 note "implicitly declared private here"
//@ diag: 47:6 error "'Priv' is a private member of 'A'"
//@ diag: 27:10 note "implicitly declared private here"
class A {
  struct Priv { struct X {}; typedef int T; };
  typedef int I;
  enum E { e };
  template<class> struct PT { };
public:
  struct Pub { };
};
A::Priv p1;
A::Priv::X p2;
A::I i1 = 0;
A::E e1;
A::PT<int> *pt;
typename A::Priv *tp;
enum A::E *pe;
using AI = A::I;
void g() {
  A::Pub ok;
  (void)ok;
  struct A::Priv *pp = 0;
  (void)pp;
  A::Priv::T t = 0;
  (void)t;
}
