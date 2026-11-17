// [bit.cast]/2: a result bit from a padding bit of the source (the bits of
// a byte outside every bit-field) is indeterminate; unless the smallest
// object containing it is an `unsigned char` or `std::byte`, the cast has
// undefined behavior and is not a constant ([expr.const]). An `unsigned
// char` bit-field so initialized holds an indeterminate value that may not
// be read. A base class with a pointer member is still excluded.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
//@ diag: 37:20 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 37:24 note "indeterminate value can only initialize an object of type 'unsigned char', 'char', or 'std::byte'; 'unsigned int' is invalid"
//@ diag: 41:20 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 41:24 note "indeterminate value can only initialize an object of type 'unsigned char', 'char', or 'std::byte'; 'int' is invalid"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:28 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 45:15 note "in call to 'c()'"
//@ diag: 48:18 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 48:18 note "subobject 'b' is not initialized"
//@ diag: 47:31 note "subobject declared here"
//@ diag: 52:25 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 52:29 note "bit_cast from a pointer type is not allowed in a constant expression"
//@ diag: 50:17 note "invalid type 'int *' is a member of 'HasPtr'"
//@ diag: 51:17 note "invalid type 'HasPtr' is a base of 'Holder'"
//@ diag: 59:16 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 59:35 note "indeterminate value can only initialize an object of type 'unsigned char', 'char', or 'std::byte'; 'unsigned int' is invalid"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:35 note "indeterminate value can only initialize an object of type 'unsigned char', 'char', or 'std::byte'; 'unsigned int' is invalid"
//@ diag: 60:15 note "in call to 'gap4()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:31 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'gap1()'"
struct Low { unsigned v : 4; unsigned char rest[3]; };
constexpr unsigned a = __builtin_bit_cast(unsigned, Low{9, {1, 2, 3}});

struct Padded { unsigned char a; short s; };
struct IntField { unsigned char x; int y : 8; short s; };
constexpr IntField b = __builtin_bit_cast(IntField, Padded{3, 4});

struct CharField { unsigned char x : 8; unsigned char y : 8; short s; };
constexpr int c() { return __builtin_bit_cast(CharField, Padded{3, 4}).y; }
static_assert(c() == 0);

struct Bytes4 { unsigned char b[4]; };
constexpr Bytes4 d = __builtin_bit_cast(Bytes4, Low{9, {1, 2, 3}});

struct HasPtr { int *p; };
struct Holder : HasPtr {};
constexpr unsigned long e = __builtin_bit_cast(unsigned long, Holder{});

// An unnamed bit-field's bits are padding ([class.bit]/2) in the source,
// so the destination member made of them is indeterminate.
struct Named { unsigned char a : 8, b : 8; unsigned short h; unsigned int f : 8, g : 24; };
struct Gap1 { unsigned char a : 1, : 7, b : 8; unsigned short h; unsigned int f : 8, g : 24; };
struct Gap4 { unsigned char a : 8, b : 8; unsigned short h; unsigned int f : 1, : 7, g : 24; };
constexpr bool gap4() { Named n = __builtin_bit_cast(Named, Gap4{1, 2, 0, 1, 3}); return n.g == 3; }
static_assert(gap4());
constexpr int gap1() { return __builtin_bit_cast(Named, Gap1{1, 2, 0, 3, 4}).a; }
static_assert(gap1() == 1);
