// [dcl.enum]/2, [dcl.typedef]: an alias-declaration whose type is
// invalid declares nothing, so an enum-base naming it is an unknown
// type name too; constant evaluation of a function that builds a value
// of that enumeration then stops at the non-constexpr call. kiln keeps
// the alias with an error type and its Oven aborts on the enumeration's
// conversion (internal compiler error). Reduced from real code.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): WP-2141, an enumeration whose base is an invalid alias has no underlying type and the Oven's convert ICEs on E{1}
//@ diag: 16:16 error "unknown type name 'undeclared_size_t'"
//@ diag: 17:16 error "unknown type name 'size_t'"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:3 note "non-constexpr function 'make' cannot be used in a constant expression"
//@ diag: 23:15 note "in call to 'f()'"
//@ diag: 18:7 note "declared here"
using size_t = undeclared_size_t;
enum class E : size_t {};
void *make(E);
constexpr int f() {
  make(E{1});
  return 0;
}
static_assert(f() == 0);
