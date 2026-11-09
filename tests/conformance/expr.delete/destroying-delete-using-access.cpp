// [expr.delete]/12, [namespace.udecl]/19, [class.dtor]/16 (WP-1089): a
// destroying operator delete found through a using-declaration is
// access-checked as the using-declaration (public here), not as the
// member it names; converting the operand, or `this` in a virtual
// destructor, to its first parameter through a private base is the error.
// (At namespace scope clang prints the classes fully scoped, which kiln's
// base-access diagnostic does not yet: kept global.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:30 error "cannot cast 'C' to its private base class 'A'"
//@ diag: 28:12 note "declared private here"
//@ diag: 22:9 error "cannot cast 'D' to its private base class 'A'"
//@ diag: 28:12 note "declared private here"
//@ diag: 32:24 note "while checking implicit 'delete this' for virtual destructor"
namespace std {
struct destroying_delete_t {
  explicit destroying_delete_t() = default;
};
}
struct A {
  void operator delete(
      A*,
      std::destroying_delete_t);
};
struct OK : A { using A::operator delete; };
void delete_ok(OK *p) { delete p; }
struct OKD : OK { virtual ~OKD() {} };
struct B : private A { using A::operator delete; };
struct C : B {};
void delete_C(C *c) { delete c; }

struct D : B { virtual ~D() {} };
