// [expr.new]/27: `new T[n]` potentially invokes T's destructor: a
// deleted one is an error and a private one is diagnosed as for a
// temporary, both at the allocated type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:16 error "attempt to use a deleted function"
//@ diag: 13:13 note "destructor of 'U' is implicitly deleted because variant field 's' has a non-trivial destructor"
//@ diag: 18:16 error "temporary of type 'P' has private destructor"
//@ diag: 14:11 note "implicitly declared private here"
//@ diag: 19:16 error "attempt to use a deleted function"
//@ diag: 15:12 note "'~D' has been explicitly marked deleted here"
struct S { ~S(); };
union U { S s; int i; U(); };
class P { ~P(); public: P(); };
struct D { ~D() = delete; };
void f(int n) {
  auto a = new U[n];
  auto b = new P[n];
  auto c = new D[2];
  auto d = new D;
}
