// [expr.sub]/2, [expr.add]/4, [expr.shift]/1, [expr.const]/5.15: a wide
// `_BitInt(N)` (N > 128) index or pointer offset out of its array, a shift
// count out of range for the narrow left operand's type, and a conversion
// between a wide integer and a pointer (a `reinterpret_cast` of any kind)
// are not core constant expressions (spec/oven.md "`_BitInt`"). clang is
// the oracle; x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:15 error "constexpr variable 'i1' must be initialized by a constant expression"
//@ diag: 42:20 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 43:15 error "constexpr variable 'i2' must be initialized by a constant expression"
//@ diag: 43:26 note "cannot refer to element -1 of array of 3 elements in a constant expression"
//@ diag: 44:22 error "constexpr variable 'i3' must be initialized by a constant expression"
//@ diag: 44:31 note "cannot refer to element 4 of array of 3 elements in a constant expression"
//@ diag: 45:22 error "constexpr variable 'i4' must be initialized by a constant expression"
//@ diag: 45:31 note "cannot refer to element -1 of array of 3 elements in a constant expression"
//@ diag: 46:15 error "constexpr variable 's1' must be initialized by a constant expression"
//@ diag: 46:22 note "shift count 32 >= width of type 'int' (32 bits)"
//@ diag: 47:15 error "constexpr variable 's2' must be initialized by a constant expression"
//@ diag: 47:22 note "negative shift count -2"
//@ diag: 49:15 error "constexpr variable 'c1' must be initialized by a constant expression"
//@ diag: 48:44 note "cannot refer to element 5 of array of 3 elements in a constant expression"
//@ diag: 49:20 note "in call to 'f1()'"
//@ diag: 50:35 warning "shift count >= width of type [-Wshift-count-overflow]"
//@ diag: 51:15 error "constexpr variable 'c2' must be initialized by a constant expression"
//@ diag: 50:35 note "shift count 40 >= width of type 'int' (32 bits)"
//@ diag: 51:20 note "in call to 'f2()'"
//@ diag: 52:16 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 52:21 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 53:13 error "constexpr variable 'r2' must be initialized by a constant expression"
//@ diag: 53:18 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 54:13 error "constexpr variable 'r3' must be initialized by a constant expression"
//@ diag: 54:18 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 55:16 error "constexpr variable 'r4' must be initialized by a constant expression"
//@ diag: 55:21 note "reinterpret_cast is not allowed in a constant expression"

using S = _BitInt(200);
constexpr S big = S(1) << 150;
constexpr int arr[3] = {1, 2, 3};

constexpr int i1 = arr[S(3)];
constexpr int i2 = S(-1)[arr];
constexpr const int *i3 = arr + S(4);
constexpr const int *i4 = arr - S(1);
constexpr int s1 = 1 << S(32);
constexpr int s2 = 1 >> S(-2);
constexpr int f1() { const int *p = arr; p += S(5); return 0; }
constexpr int c1 = f1();
constexpr int f2() { int x = 1; x <<= S(40); return x; }
constexpr int c2 = f2();
constexpr int *r1 = (int *)S(0);
constexpr S r2 = (S)&arr[0];
constexpr S r3 = reinterpret_cast<S>(big);
constexpr int *r4 = reinterpret_cast<int *>(big);
