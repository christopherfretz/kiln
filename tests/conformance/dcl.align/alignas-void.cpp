// [dcl.align]/3: alignas(type-id) is alignas(alignof(type-id)), so the
// type-id must be a complete object type; void and an incomplete class are
// rejected, complete types stay valid. Found by the clang negative-test run
// (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "invalid application of 'alignas' to an incomplete type 'void'"
//@ diag: 13:16 error "invalid application of 'alignas' to an incomplete type 'Inc'"
//@ diag: 11:8 note "forward declaration of 'Inc'"
struct Inc;
class alignas(void) C {};
struct alignas(Inc) D {};
struct alignas(int) E {};
struct alignas(double) F { char c; };
struct alignas(F) G {};
static_assert(alignof(G) == alignof(double));
