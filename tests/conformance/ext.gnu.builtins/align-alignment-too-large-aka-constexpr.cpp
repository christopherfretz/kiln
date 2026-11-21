// [expr.const]: the note for an alignment too large for the source type of
// `__builtin_align_up` names a typedef'd source type with its `aka`, as
// clang does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:13 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 11:65 note "requested alignment must be 2147483648 or less for type 'I' (aka 'int'); 1099511627776 is invalid"
//@ diag: 12:17 note "in call to 'f(3, 1099511627776)'"

using I = int;
constexpr I f(I x, long long al) { return __builtin_align_up(x, al); }
constexpr I a = f(3, 1LL << 40);
