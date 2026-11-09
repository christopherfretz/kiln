// [expr.const]/10.8, [expr.add]/4, [expr.ref]: member access and member calls
// through a pointer past the end of an array or a subobject, via '->' and
// via an element past the end (WP-948 siblings of member-of-past-end-base).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 16:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 16:30 note "cannot access field of pointer past the end of object"
//@ diag: 17:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 17:26 note "member call on dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 18:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 18:34 note "member call on dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 21:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 21:31 note "cannot access field of pointer past the end of object"
struct S { int x; constexpr int get() const { return x; } };
constexpr S arr[2] = {{1}, {2}};
constexpr int a = (arr + 2)->x;
constexpr int b = arr[2].get();
constexpr int c = (&arr[1] + 1)->get();
struct T { S s; int y; };
constexpr T t{{3}, 4};
constexpr int d = (&t.s + 1)->x;
