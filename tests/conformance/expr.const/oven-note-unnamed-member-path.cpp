// [expr.const], [class.union.anon]: the "in call to" note prints a pointer
// argument's path through an anonymous union by naming the unnamed member
// the way clang does (`v.V::(anonymous union at FILE:L:C).data[1]`), not
// as an empty name (`v..data[1]`).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:42 note "cannot refer to element 6 of array of 4 elements in a constant expression"
//@ diag: 18:33 note "oven-note-unnamed-member-path.cpp:16:12).data[1])'"
//@ diag: 19:15 note "in call to 'f()'"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:40 note "cannot refer to element 2 of non-array object in a constant expression"
//@ diag: 22:44 note "oven-note-unnamed-member-path.cpp:20:12).x)'"
//@ diag: 23:15 note "in call to 'h()'"
struct V { union { int data[4]; }; int n; constexpr V() : data{1,2,3,4}, n(0) {} };
constexpr int get(const int *p) { return p[5]; }
constexpr int f() { V v; return get(&v.data[1]); }
static_assert(f() == 0);
struct W { union { int x; float y; }; };
constexpr int g(const int *p) { return p[2]; }
constexpr int h() { W w{}; w.x = 1; return g(&w.x); }
static_assert(h() == 0);
