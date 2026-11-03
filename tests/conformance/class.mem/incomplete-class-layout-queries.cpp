// [class.mem.general]/8, [basic.types.general]/5, [expr.sizeof]/1,
// [expr.alignof]/1, [support.types.layout]/1: inside its own
// member-specification a class is incomplete, so asking its size,
// alignment or a member offset is diagnosed and never computes (or caches)
// a layout (kiln_abi::ensure_layout refuses an incomplete class,
// spec/abi.md §1; WP-471). The constant evaluation that names `S*` meanwhile
// keeps S opaque, and S is laid out with every member at `}`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:17 error "invalid application of 'sizeof' to an incomplete type 'S'"
//@ diag: 17:8 note "definition of 'S' is not complete until the closing '}'"
//@ diag: 20:17 error "invalid application of 'alignof' to an incomplete type 'S'"
//@ diag: 17:8 note "definition of 'S' is not complete until the closing '}'"
//@ diag: 21:17 error "offsetof of incomplete type 'S'"
//@ diag: 17:8 note "definition of 'S' is not complete until the closing '}'"
//@ std: c++20
struct S {
  int a;
  static_assert(sizeof(S) == 4);
  static_assert(alignof(S) == 4);
  static_assert(__builtin_offsetof(S, a) == 0);
  static constexpr S *p = nullptr;
  static_assert(!p);
  long b;
};
static_assert(sizeof(S) >= sizeof(int) + sizeof(long));
static_assert(__builtin_offsetof(S, b) >= sizeof(int));
