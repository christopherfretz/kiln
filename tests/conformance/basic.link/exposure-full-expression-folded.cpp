// [basic.link]/14, /17 (WP-1847): a declaration names what its expressions
// name, whatever their full-expression folds to; /14 ignores only a
// reference to a TU-local constant that is not an odr-use (/14.4), and no
// CWG issue exempts a folded expression. So in a module interface an
// inline function whose body names a TU-local function or variable in a
// discarded-value expression (`helper;`, `(void)&hidden;`), on a branch of
// a conditional that is never taken, or odr-uses a TU-local constant in a
// comparison that folds (`&K != nullptr`), is an exposure, and so is a
// class whose default member initializer folds the same way (the error at
// the class). GCC 16.1 folds the first five away before its check and
// accepts them (it rejects the last two); clang 22 only warns (a
// deviation, docs/status/deviations.md).
// A constant read without an odr-use stays valid
// (exposure-names-tu-local-accepted.cpp).
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ strict-diags
//@ diag: 32:19 error "declaration of 'discarded_name' exposes TU-local entity 'helper'"
//@ diag: 33:19 error "declaration of 'discarded_variable' exposes TU-local entity 'hidden'"
//@ diag: 34:19 error "declaration of 'discarded_address' exposes TU-local entity 'hidden'"
//@ diag: 35:19 error "declaration of 'branch_not_taken' exposes TU-local entity 'helper'"
//@ diag: 36:49 warning "comparison of address of 'K' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 36:20 error "declaration of 'compared_address' exposes TU-local entity 'K'"
//@ diag: 37:22 error "declaration of 'constant_branch' exposes TU-local entity 'helper'"
//@ diag: 38:15 error "declaration of 'Member' exposes TU-local entity 'helper'"
export module M;
static int helper() { return 1; }
static int hidden;
static const int K = 4;

export inline int discarded_name() { (void)helper; return 0; }
export inline int discarded_variable() { (void)hidden; return 0; }
export inline int discarded_address() { (void)&hidden; return 0; }
export inline int branch_not_taken() { return 0 ? helper() : 1; }
export inline bool compared_address() { return &K != nullptr; }
export constexpr int constant_branch() { return sizeof(int) ? 1 : helper(); }
export struct Member { int m = 0 ? helper() : 1; };
