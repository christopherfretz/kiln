// [dcl.fct.spec]/4, [expr.const]/12, [temp.inst]/3: the operand of an
// explicit-specifier is a contextually converted constant expression of type
// bool. A class type converts through a constexpr conversion function (also
// in a noexcept-specifier, [except.spec]/2); a non-constant operand, a type
// with no conversion and a narrowing are errors. An invalid
// explicit-specifier in an instantiated member drops the member, as clang.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:12 error "explicit specifier argument evaluates to 2, which cannot be narrowed to type 'bool'"
//@ diag: 37:12 error "explicit specifier argument is not a constant expression"
//@ diag: 37:12 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 34:5 note "declared here"
//@ diag: 38:12 error "value of type 'NoConv' is not implicitly convertible to 'bool'"
//@ diag: 39:12 error "conversion from 'double' to 'bool' is not allowed in a converted constant expression"
//@ diag: 40:12 error "explicit specifier argument is not a constant expression"
//@ diag: 40:12 note "non-constexpr function 'operator bool' cannot be used in a constant expression"
//@ diag: 31:17 note "declared here"
//@ diag: 41:12 error "value of type 'E' is not implicitly convertible to 'bool'"
//@ diag: 46:12 error "value of type 'NoConv' is not implicitly convertible to 'bool'"
//@ diag: 52:11 note "in instantiation of template class 'B<NoConv>' requested here"
//@ diag: 52:11 error "no viable conversion from 'int' to 'B<NoConv>'"
//@ diag: 45:26 note "candidate constructor (the implicit copy constructor) not viable"
//@ diag: 45:26 note "candidate constructor (the implicit move constructor) not viable"
//@ diag: 46:12 error "explicit specifier argument is not a constant expression"
//@ diag: 53:11 note "in instantiation of template class 'B<RtConv>' requested here"
//@ diag: 46:12 note "non-constexpr function 'operator bool' cannot be used in a constant expression"
//@ diag: 31:17 note "declared here"
//@ diag: 53:11 error "excess elements in struct initializer"
struct NoConv {};
struct RtConv { operator bool() const { return true; } };
struct CConv { constexpr explicit operator bool() const { return false; } };
enum class E { a };
int n = 1;
struct A {
  explicit(1 + 1) A(int);
  explicit(n) A(long);
  explicit(NoConv{}) A(char);
  explicit(1.0) A(short);
  explicit(RtConv{}) A(float);
  explicit(E::a) A(double);
  explicit(CConv{}) A(unsigned);
};
A a1 = 1u;
template<class T> struct B {
  explicit(T{}) B(int);
};
template<bool V> struct bc { constexpr operator bool() const { return V; } };
void f() noexcept(bc<true>());
static_assert(noexcept(f()));
B<CConv> b1 = 1;
B<NoConv> b2 = 1;
B<RtConv> b3(1);
