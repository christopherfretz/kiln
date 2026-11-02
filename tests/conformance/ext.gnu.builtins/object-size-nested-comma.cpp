// __builtin_object_size through NESTED comma operators in a constant
// expression (clang 19 tryEvaluateBuiltinObjectSize): the operand is
// evaluated ignoring side effects, so a comma's left operand at any depth
// (parenthesized or not, on either side of the array-to-pointer decay, in
// a subscript or an addend) is an ignored value whose failure is noted;
// after it, any lvalue-to-rvalue read fails the evaluation and the result
// is the default ([expr.comma]/1, [expr.const]/10). Verified against
// clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 46:83 warning "left operand of comma operator has no effect"
int ext();
constexpr int cx() { return 1; }
constexpr void inc(int &x) { ++x; }
struct S { int a; char c[8]; };
// Nested commas on the operand's spine.
constexpr long n1() { int a[4] = {}; return __builtin_object_size((a[0] = 1, (a[1] = 2, a + 1)), 0) * 100 + a[0] + a[1]; }
static_assert(n1() == 1200);
constexpr long n2() { char b[6] = {}; return __builtin_object_size((ext(), (ext(), b)), 0); }
static_assert(n2() == 6);
constexpr long n3() { char b[6] = {}; return __builtin_object_size(((ext(), ext()), b), 0); }
static_assert(n3() == 6);
constexpr long n4() { char b[6] = {}; int i = 0; return __builtin_object_size((i++, (ext(), b + 2)), 0) * 10 + i; }
static_assert(n4() == 40);
constexpr long n5() { char b[6] = {}; return __builtin_object_size((ext(), ((ext(), (b)))), 1); }
static_assert(n5() == 6);
constexpr long n6() { char b[6] = {}; return __builtin_object_size((void*)(ext(), (char*)(ext(), b + 1)), 0); }
static_assert(n6() == 5);
constexpr long n7() { S s = {}; return __builtin_object_size((ext(), (ext(), s.c)), 1); }
static_assert(n7() == 8);
constexpr long n8() { S s = {}; return __builtin_object_size((ext(), (ext(), &s.c[3])), 1); }
static_assert(n8() == 5);
// Commas inside a subscript or an addend; the modification is not done.
constexpr long m1() { char b[6] = {}; return __builtin_object_size(&b[(ext(), 1)], 0); }
static_assert(m1() == 5);
constexpr long m2() { char b[6] = {}; return __builtin_object_size(b + (ext(), 2), 0); }
static_assert(m2() == 4);
constexpr long m3() { char b[6] = {}; int i = 0; return __builtin_object_size(&b[(i++, 1)], 0) * 10 + i; }
static_assert(m3() == 50);
constexpr long m4() { S s[2] = {}; return __builtin_object_size(&s[(ext(), 1)].c[(ext(), 2)], 1); }
static_assert(m4() == 6);
constexpr long m5() { char b[6] = {}; return __builtin_object_size((ext(), b) + (ext(), 1), 0); }
static_assert(m5() == 5);
constexpr long m6() { char b[6] = {}; return __builtin_object_size(&b[(ext(), (ext(), 3))], 0); }
static_assert(m6() == 3);
constexpr long m7() { char b[6] = {}; int i = 2; return __builtin_object_size(&b[(i, (ext(), 1))], 0); }
static_assert(m7() == 5);
// A read after an ignored failure fails: the default. A read before it,
// or after a left operand that succeeds, does not.
constexpr long r1() { char b[6] = {}; char *p = b; return __builtin_object_size((ext(), p) + 1, 0); }
static_assert(r1() == -1);
constexpr long r2() { char b[6] = {}; char *p = b; return __builtin_object_size((ext(), p + 1), 0); }
static_assert(r2() == -1);
constexpr long r3() { char b[6] = {}; const int i = 1; return __builtin_object_size(&b[(ext(), i)], 0); }
static_assert(r3() == -1);
constexpr long r4() { char b[6] = {}; return __builtin_object_size(&b[(ext(), b[0])], 0); }
static_assert(r4() == -1);
constexpr long r5() { char b[6] = {}; char *p = b; int i = 0; return __builtin_object_size((i++, p), 0); }
static_assert(r5() == -1);
constexpr long r6(int k) { char b[6] = {}; return __builtin_object_size(&b[(ext(), k)], 0); }
static_assert(r6(1) == -1);
constexpr long r7() { char b[6] = {}; char *p = b; int i = 0; return __builtin_object_size((inc(i), p), 0) * 10 + i; }
static_assert(r7() == -10);
constexpr long r8() { char b[6] = {}; int i = 1; return __builtin_object_size(&b[i + (ext(), 0)], 0); }
static_assert(r8() == 5);
constexpr long r9() { char b[6] = {}; char *p = b; return __builtin_object_size((cx(), p) + 1, 0); }
static_assert(r9() == 5);
constexpr long r10() { char b[6] = {}; int i = 0; return __builtin_object_size((inc(i), b + 1), 0) * 10 + i; }
static_assert(r10() == 50);
constexpr long r11() { char b[6] = {}; return __builtin_object_size(&b[(ext(), cx())], 0); }
static_assert(r11() == 5);
