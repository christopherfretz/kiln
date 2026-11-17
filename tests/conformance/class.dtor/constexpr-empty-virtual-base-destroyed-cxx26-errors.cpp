// [class.dtor]/13, [expr.const]/10 (C++26, P3533R2): destroying a
// complete object runs the destructors of its virtual bases, empty ones
// included, so a non-constexpr call there makes the evaluation not
// constant: for a local, an array element, a `new` object, a base
// class's virtual base, an object destroyed by unwinding, and the
// constant destruction of a constexpr variable. kiln skipped the
// destructor of an empty virtual base. GCC 16.1 skips it too; the text
// decides. The accepted side is constexpr-empty-virtual-base-destroyed-cxx26.cpp.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:60 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 52:35 note "in call to 'b.~E()'"
//@ diag: 52:35 note "in call to 'b.~Bar()'"
//@ diag: 60:15 note "in call to 'local()'"
//@ diag: 48:6 note "declared here"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:60 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 53:34 note "in call to 'd.~E()'"
//@ diag: 53:34 note "in call to 'd.~Der()'"
//@ diag: 61:15 note "in call to 'base()'"
//@ diag: 48:6 note "declared here"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:60 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 54:49 note "in call to '{*new Der<true>#0}.~E()'"
//@ diag: 54:49 note "in call to '{*new Der<true>#0}.~Der()'"
//@ diag: 62:15 note "in call to 'heap()'"
//@ diag: 48:6 note "declared here"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:60 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 55:35 note "in call to 'a[1].~E()'"
//@ diag: 55:35 note "in call to 'a[1].~Bar()'"
//@ diag: 63:15 note "in call to 'array()'"
//@ diag: 48:6 note "declared here"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:60 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 57:19 note "in call to 'b.~E()'"
//@ diag: 57:19 note "in call to 'b.~Bar()'"
//@ diag: 64:15 note "in call to 'unwind()'"
//@ diag: 48:6 note "declared here"
//@ diag: 66:21 error "constexpr variable 'bad' must have constant destruction"
//@ diag: 49:60 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 66:21 note "in call to 'bad.~E()'"
//@ diag: 66:21 note "in call to 'bad.~Bar()'"
//@ diag: 48:6 note "declared here"
void nc();
template <bool Fail> struct E { constexpr ~E() { if (Fail) nc(); } };
template <bool Fail> struct Bar : virtual E<Fail> { int v = 1; };
template <bool Fail> struct Der : Bar<Fail> {};
constexpr int local() { Bar<true> b; return b.v; }
constexpr int base() { Der<true> d; return d.v; }
constexpr int heap() { auto *p = new Der<true>; delete p; return 1; }
constexpr int array() { Bar<true> a[2]; return 1; }
constexpr int unwind() {
  try { Bar<true> b; throw 1; } catch (int) {}
  return 1;
}
static_assert(local() == 1);
static_assert(base() == 1);
static_assert(heap() == 1);
static_assert(array() == 1);
static_assert(unwind() == 1);
constexpr Bar<false> ok;
constexpr Bar<true> bad;
