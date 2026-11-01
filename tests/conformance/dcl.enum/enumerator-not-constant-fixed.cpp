// [dcl.enum]/5: with a fixed underlying type (a scoped enumeration's
// implicit int included) an enumerator's initializer is a converted
// constant expression, never folded: 'enumerator value is not a constant
// expression'; without one it may fold (clang CheckEnumConstant; WP-230)
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wgnu-folding-constant
//@ diag: 18:21 error "enumerator value is not a constant expression"
//@ diag: 18:26 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 17:14 note "declared here"
//@ diag: 19:20 error "enumerator value is not a constant expression"
//@ diag: 19:25 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 17:14 note "declared here"
//@ diag: 20:14 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 20:19 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 17:14 note "declared here"
const double d = 2.0;
enum class EC { x = (int)d };
enum F : int { y = (int)d };
enum G { g = (int)d };
