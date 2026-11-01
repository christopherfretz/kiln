// [namespace.udecl]/19: every member a using-declarator names must be
// accessible; [class.access]/4: so must a type named by a mem-initializer-id.
// An inheriting constructor's using-declarator is not checked.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:12 error "'Priv' is a private member of 'A'"
//@ diag: 17:10 note "implicitly declared private here"
//@ diag: 31:12 error "'f' is a private member of 'A'"
//@ diag: 18:8 note "implicitly declared private here"
//@ diag: 32:12 error "'v' is a private member of 'A'"
//@ diag: 19:7 note "implicitly declared private here"
//@ diag: 33:12 error "'g' is a private member of 'A'"
//@ diag: 25:8 note "declared private here"
//@ diag: 35:9 error "'Self' is a private member of 'A'"
//@ diag: 20:13 note "implicitly declared private here"
class A {
  struct Priv { };
  void f();
  int v;
  typedef A Self;
public:
  A() { }
  void g(int);
private:
  void g(char);
protected:
  struct Prot { };
};
struct D : A {
  using A::Priv;
  using A::f;
  using A::v;
  using A::g;
  using A::Prot;
  D() : Self() { }
};
struct E : A { using A::A; };
