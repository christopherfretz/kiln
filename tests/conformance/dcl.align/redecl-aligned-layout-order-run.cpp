// [dcl.align], [basic.align]/1, GNU `aligned` on a redeclaration: a
// declaration carries its own alignment attributes and the strictest one
// of the declarations before it (clang `mergeAlignedAttrs`), never a later
// declaration's. A class or enumeration is laid out from its definition,
// so `struct __attribute__((aligned(16))) C;` after a definition with
// `aligned(8)` (kept: the definition has an `aligned`, clang
// `checkNewAttributesAfterDef`) changes nothing; a typedef's alignment is
// that of the declaration its use names, so a field declared with the
// 8-aligned `R` stays 8-aligned after `R` is redeclared 16-aligned, and a
// plain `typedef int R;` keeps the 8; a variable's `__alignof__` reads
// the declaration the name finds. Each answer is the same whether or not
// the class was laid out before the redeclaration (kiln before WP-2830
// gave 32 and 16 when the first layout came after it). Reduced from real
// code; values checked against clang 19 and clang 22.
//@ expect: run

// Class: laid out (static_assert) before the redeclaration...
struct __attribute__((aligned(8))) C1 { int x; };
struct S1 { char c; C1 m; };
static_assert(sizeof(S1) == 16);
struct __attribute__((aligned(16))) C1;
static_assert(sizeof(S1) == 16);
static_assert(alignof(C1) == 8);

// ...and first laid out after it.
struct __attribute__((aligned(8))) C2 { int x; };
struct S2 { char c; C2 m; };
struct __attribute__((aligned(16))) C2;
static_assert(sizeof(S2) == 16);
static_assert(alignof(C2) == 8);

// An `aligned` on a declaration before the definition is inherited.
struct C3;
struct __attribute__((aligned(16))) C3;
struct C3 { int x; };
static_assert(alignof(C3) == 16);

// Enumeration, both orders.
enum __attribute__((aligned(8))) E1 : int { A1 };
struct SE1 { char c; E1 e; };
static_assert(sizeof(SE1) == 16);
enum __attribute__((aligned(16))) E1 : int;
static_assert(sizeof(SE1) == 16);
static_assert(alignof(E1) == 8);

enum __attribute__((aligned(8))) E2 : int { A2 };
struct SE2 { char c; E2 e; };
enum __attribute__((aligned(16))) E2 : int;
static_assert(sizeof(SE2) == 16);
static_assert(alignof(E2) == 8);

// Typedef redeclared with a stricter alignment, both orders: the field
// names the first declaration; `R1` named afterwards is the second.
typedef int R1 __attribute__((aligned(8)));
struct T1 { char c; R1 r; };
static_assert(sizeof(T1) == 16);
typedef int R1 __attribute__((aligned(16)));
static_assert(sizeof(T1) == 16);
static_assert(alignof(R1) == 16);

typedef int R2 __attribute__((aligned(8)));
struct T2 { char c; R2 r; };
typedef int R2 __attribute__((aligned(16)));
static_assert(sizeof(T2) == 16);
static_assert(alignof(R2) == 16);

// A plain redeclaration inherits the alignment; a lower one keeps the
// stricter earlier one; a typedef's own `aligned` may lower it.
typedef int R3 __attribute__((aligned(8)));
typedef int R3;
static_assert(alignof(R3) == 8);
struct T3 { char c; R3 r; };
static_assert(sizeof(T3) == 16);
typedef int R4 __attribute__((aligned(8)));
typedef int R4 __attribute__((aligned(2)));
static_assert(alignof(R4) == 8);
typedef int R5 __attribute__((aligned(2)));
typedef int R5;
static_assert(alignof(R5) == 2);

// Variable: the definition keeps its 8; the name after the stricter
// redeclaration finds a 16-aligned declaration.
int v __attribute__((aligned(8))) = 1;
unsigned long align_v_before() { return __alignof__(v); }
extern int v __attribute__((aligned(16)));
unsigned long align_v_after() { return __alignof__(v); }

unsigned long size_s2() { return sizeof(S2); }
unsigned long size_t2() { return sizeof(T2); }

int main() {
  if (size_s2() != 16 || size_t2() != 16 || alignof(C2) != 8)
    return 1;
  if (align_v_before() != 8 || align_v_after() != 16)
    return 2;
  S2 a{'a', {7}};
  S2 b = a;
  T2 t{'b', 9};
  T2 u = t;
  if (b.c != 'a' || b.m.x != 7 || u.c != 'b' || u.r != 9)
    return 3;
  return v == 1 ? 0 : 4;
}
