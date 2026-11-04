// [dcl.attr.grammar]/6, [dcl.attr.unknown]: `reentrant_capability` joined
// clang's thread-safety family in clang 22 (WP-606; spec/sema.md §4.5); at
// `-fclang-compat=19` it is an unknown attribute in both spellings, while the
// rest of the family stays known (thread-safety-attributes-gnu.cpp uses it
// under clang 22).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
//@ diag: 11:23 warning "unknown attribute 'reentrant_capability' ignored [-Wunknown-attributes]"
//@ diag: 12:10 warning "unknown attribute 'reentrant_capability' ignored [-Wunknown-attributes]"
struct __attribute__((reentrant_capability, capability("mutex"))) A {};
struct [[clang::reentrant_capability, clang::capability("mutex")]] B {};
static_assert(!__has_attribute(reentrant_capability));
static_assert(!__has_cpp_attribute(clang::reentrant_capability));
static_assert(__has_attribute(capability));
