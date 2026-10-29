// [bit.cast]/3, [expr.const]/5: __builtin_bit_cast is not a constant when a
// result bit would be indeterminate (padding), or when the source holds a
// pointer or a union.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:30 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 14:34 note "indeterminate value can only initialize an object of type 'unsigned char', 'char', or 'std::byte'; 'unsigned long long' is invalid"
//@ diag: 16:25 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 16:29 note "bit_cast from a pointer type is not allowed in a constant expression"
//@ diag: 19:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 19:19 note "bit_cast from a union type is not allowed in a constant expression"
struct Pad { char c; int i; };
constexpr Pad pad = {1, 2};
constexpr unsigned long long a = __builtin_bit_cast(unsigned long long, pad);
constexpr int x = 0;
constexpr unsigned long b = __builtin_bit_cast(unsigned long, &x);
union U { int i; float f; };
constexpr U u = {1};
constexpr int c = __builtin_bit_cast(int, u);
