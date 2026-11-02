// [dcl.type.class.deduct]/1, [over.match.class.deduct]/1: the "no viable
// constructor or deduction guide" error names the placeholder's template the
// way it was first spelled: qualified names stay scoped (inline namespaces
// elided unless written), an unqualified name stays bare.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:15 error "no viable constructor or deduction guide for deduction of template arguments of 'std::vector'"
//@ diag: 44:65 note "candidate template ignored: could not match 'vector<T>' against 'int'"
//@ diag: 44:65 note "implicit deduction guide declared as 'template <class T> vector(vector<T>) -> vector<T>'"
//@ diag: 44:74 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 44:74 note "implicit deduction guide declared as 'template <class T> vector(T, int) -> vector<T>'"
//@ diag: 49:8 error "no viable constructor or deduction guide for deduction of template arguments of 'M::V'"
//@ diag: 45:63 note "candidate template ignored: could not match 'V<T>' against 'int'"
//@ diag: 45:63 note "implicit deduction guide declared as 'template <class T> V(V<T>) -> V<T>'"
//@ diag: 45:67 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 45:67 note "implicit deduction guide declared as 'template <class T> V(T, int) -> V<T>'"
//@ diag: 50:8 error "no viable constructor or deduction guide for deduction of template arguments of 'S::U'"
//@ diag: 46:68 note "candidate template ignored: could not match 'U<T>' against 'int'"
//@ diag: 46:68 note "implicit deduction guide declared as 'template <class T> U(U<T>) -> U<T>'"
//@ diag: 46:72 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:72 note "implicit deduction guide declared as 'template <class T> U(T, int) -> U<T>'"
//@ diag: 54:9 error "no viable constructor or deduction guide for deduction of template arguments of 'N::I::B'"
//@ diag: 52:61 note "candidate template ignored: could not match 'B<T>' against 'int'"
//@ diag: 52:61 note "implicit deduction guide declared as 'template <class T> B(B<T>) -> B<T>'"
//@ diag: 52:65 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 52:65 note "implicit deduction guide declared as 'template <class T> B(T, int) -> B<T>'"
//@ diag: 55:6 error "no viable constructor or deduction guide for deduction of template arguments of 'N::I::B'"
//@ diag: 52:61 note "candidate template ignored: could not match 'B<T>' against 'int'"
//@ diag: 52:61 note "implicit deduction guide declared as 'template <class T> B(B<T>) -> B<T>'"
//@ diag: 52:65 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 52:65 note "implicit deduction guide declared as 'template <class T> B(T, int) -> B<T>'"
//@ diag: 57:3 error "no viable constructor or deduction guide for deduction of template arguments of 'C'"
//@ diag: 53:41 note "candidate template ignored: could not match 'C<T>' against 'int'"
//@ diag: 53:41 note "implicit deduction guide declared as 'template <class T> C(C<T>) -> C<T>'"
//@ diag: 53:45 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 53:45 note "implicit deduction guide declared as 'template <class T> C(T, int) -> C<T>'"
//@ diag: 58:7 error "no viable constructor or deduction guide for deduction of template arguments of 'C'"
//@ diag: 53:41 note "candidate template ignored: could not match 'C<T>' against 'int'"
//@ diag: 53:41 note "implicit deduction guide declared as 'template <class T> C(C<T>) -> C<T>'"
//@ diag: 53:45 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 53:45 note "implicit deduction guide declared as 'template <class T> C(T, int) -> C<T>'"
//@ requires: clang<22
// clang 22 emits error: no viable constructor or deduction guide for deduction of template argument...; WP-355 residual
namespace std { inline namespace __1 { template<class T> struct vector { vector(T, int); }; } }
namespace M { inline namespace __1 { template<class T> struct V { V(T, int); }; } }
namespace S { template<class T> struct U; template<class T> struct U { U(T, int); }; }
void t() {
  std::vector v(1);
  M::V w(1);
  S::U u(1);
}
namespace N { inline namespace I { template<class T> struct B { B(T, int); }; } }
namespace M2 { template<class T> struct C { C(T, int); }; }
N::I::B b2(1);
N::B b1(1);
using namespace M2;
C c1(1);
M2::C c2(1);
