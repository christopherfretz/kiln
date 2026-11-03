// [meta.unary.prop] reference_constructs_from_temporary /
// reference_converts_from_temporary and clang's
// __reference_binds_to_temporary: T is a reference whose initialization
// from declval<U>() (direct, or copy for the converts form) is
// well-formed and binds a materialized temporary rather than directly.
// The C++23 forms take a non-reference U as a prvalue. libc++'s
// __tuple_leaf<I, T&> static_asserts on the first one (std::tie).
//@ expect: compile-ok
//@ requires: clang>=22
//@ diag: 38:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 39:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 40:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 41:15 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 42:15 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 43:15 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 44:15 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 45:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 46:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 47:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 48:15 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 49:15 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 50:16 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 66:50 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 66:50 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 67:15 note "in instantiation of variable template specialization 'dep<const int &, double>' requested here"
//@ diag: 66:50 warning "builtin __reference_binds_to_temporary is deprecated; use __reference_constructs_from_temporary instead"
//@ diag: 67:44 note "in instantiation of variable template specialization 'dep<int &, int &>' requested here"
// clang 22 deprecates __reference_binds_to_temporary and evaluates it as
// __reference_constructs_from_temporary (a non-reference U is a prvalue),
// so (const int &, int) and (int &&, int) now bind a temporary (WP-394).
struct B {};
struct D : B {};
struct ToRef { operator int &(); };
struct ToVal { operator int(); };
struct Ex { explicit operator int(); };
struct Imp { Imp(int); };

static_assert(!__reference_binds_to_temporary(int, int));
static_assert(!__reference_binds_to_temporary(int &, int &));
static_assert(!__reference_binds_to_temporary(const int &, int &));
static_assert(__reference_binds_to_temporary(const int &, int));
static_assert(__reference_binds_to_temporary(const int &, long));
static_assert(__reference_binds_to_temporary(const long &, int &));
static_assert(__reference_binds_to_temporary(int &&, int));
static_assert(!__reference_binds_to_temporary(int &, long &));
static_assert(!__reference_binds_to_temporary(const B &, D &));
static_assert(!__reference_binds_to_temporary(int &, ToRef));
static_assert(__reference_binds_to_temporary(const int &, ToVal));
static_assert(__reference_binds_to_temporary(const Imp &, int));
static_assert(!__reference_binds_to_temporary(void, int));

static_assert(__reference_constructs_from_temporary(const int &, int));
static_assert(__reference_constructs_from_temporary(int &&, int));
static_assert(!__reference_constructs_from_temporary(int &&, int &&));
static_assert(!__reference_constructs_from_temporary(const int &, int &));
static_assert(__reference_constructs_from_temporary(const int &, long &));
static_assert(!__reference_constructs_from_temporary(int, int));
static_assert(__reference_constructs_from_temporary(const B &, B));
static_assert(!__reference_constructs_from_temporary(const B &, D &));

static_assert(__reference_converts_from_temporary(const int &, int));
static_assert(!__reference_converts_from_temporary(int &&, Ex));
static_assert(__reference_converts_from_temporary(const Imp &, int));
static_assert(!__reference_converts_from_temporary(int &, ToRef));

template <class T, class U> constexpr bool dep = __reference_binds_to_temporary(T, U);
static_assert(dep<const int &, double> && !dep<int &, int &>);
int main() {}
