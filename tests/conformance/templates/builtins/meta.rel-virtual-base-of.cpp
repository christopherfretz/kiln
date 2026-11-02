// [meta.rel] (C++26 P2985): __builtin_is_virtual_base_of(Base, Derived)
// is true when Base is a virtual base class of Derived (directly or
// through a non-virtual path); a non-class or union operand gives false,
// and Derived must be complete (spec/sema-templates.md §9.1; WP-263).
// Oracle-checked by hand with clang++-22 (22.1.8): same answers and the
// same diagnostics.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 36:44 error "incomplete type 'Inc' where a complete type is required"
//@ diag: 15:8 note "forward declaration of 'Inc'"
struct A {};
struct B : virtual A {};
struct C : B {};
struct Inc;
struct D : A {};
struct E : private virtual A {};
struct F : B, virtual A {};
union U { int i; };

static_assert(__builtin_is_virtual_base_of(A, B));
static_assert(__builtin_is_virtual_base_of(A, C));
static_assert(__builtin_is_virtual_base_of(const A, volatile C));
static_assert(__builtin_is_virtual_base_of(A, E));
static_assert(__builtin_is_virtual_base_of(A, F));
static_assert(!__builtin_is_virtual_base_of(A, D));
static_assert(!__builtin_is_virtual_base_of(A, A));
static_assert(!__builtin_is_virtual_base_of(B, C));
static_assert(!__builtin_is_virtual_base_of(B, A));
static_assert(!__builtin_is_virtual_base_of(int, int));
static_assert(!__builtin_is_virtual_base_of(A &, B &));
static_assert(!__builtin_is_virtual_base_of(U, U));
static_assert(!__builtin_is_virtual_base_of(Inc, B));
template <class T, class U> constexpr bool vb = __builtin_is_virtual_base_of(T, U);
static_assert(vb<A, C> && !vb<A, D>);
bool bad = __builtin_is_virtual_base_of(A, Inc);
