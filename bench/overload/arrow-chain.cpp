// Overloaded `->` chains ([over.ref]/1, spec/sema-overload.md §6.4 and
// §11, WP-068b): fifty smart-pointer-like classes, each `operator->`
// returning the next by value, the last a raw pointer, so every `p->m`
// resolves fifty `operator->` calls (each one an operator overload
// resolution over the class's members plus the chain's cycle check).
// 12,000 member accesses through the chain (600,000 `operator->` calls). Mirrors mirrorbind's
// handle/proxy layers. Measures the `->` loop, member operator candidate
// sets and temporary materialization. No library headers.

#define KILN_10(m, p) m(p##0) m(p##1) m(p##2) m(p##3) m(p##4) \
                      m(p##5) m(p##6) m(p##7) m(p##8) m(p##9)

struct X { int m; int get() const { return m; } };

// P0::operator-> yields X*; Pn::operator-> yields P(n-1) by value.
struct P0 { X *x; X *operator->() const { return x; } };
#define KILN_LINK(n, prev) \
  struct P##n { prev q; prev operator->() const { return q; } };
KILN_LINK(1, P0)
KILN_LINK(2, P1) KILN_LINK(3, P2) KILN_LINK(4, P3) KILN_LINK(5, P4)
KILN_LINK(6, P5) KILN_LINK(7, P6) KILN_LINK(8, P7) KILN_LINK(9, P8)
KILN_LINK(10, P9) KILN_LINK(11, P10) KILN_LINK(12, P11) KILN_LINK(13, P12)
KILN_LINK(14, P13) KILN_LINK(15, P14) KILN_LINK(16, P15) KILN_LINK(17, P16)
KILN_LINK(18, P17) KILN_LINK(19, P18) KILN_LINK(20, P19) KILN_LINK(21, P20)
KILN_LINK(22, P21) KILN_LINK(23, P22) KILN_LINK(24, P23) KILN_LINK(25, P24)
KILN_LINK(26, P25) KILN_LINK(27, P26) KILN_LINK(28, P27) KILN_LINK(29, P28)
KILN_LINK(30, P29) KILN_LINK(31, P30) KILN_LINK(32, P31) KILN_LINK(33, P32)
KILN_LINK(34, P33) KILN_LINK(35, P34) KILN_LINK(36, P35) KILN_LINK(37, P36)
KILN_LINK(38, P37) KILN_LINK(39, P38) KILN_LINK(40, P39) KILN_LINK(41, P40)
KILN_LINK(42, P41) KILN_LINK(43, P42) KILN_LINK(44, P43) KILN_LINK(45, P44)
KILN_LINK(46, P45) KILN_LINK(47, P46) KILN_LINK(48, P47) KILN_LINK(49, P48)

// 12,000 accesses: 6,000 data-member and 6,000 member-function (the
// count keeps the entry above 0.5 s with clang, spec/bench.md).
#define KILN_USE(n) r += p->m + p->get();
#define KILN_100(m, p) KILN_10(m, p##0) KILN_10(m, p##1) KILN_10(m, p##2) \
                       KILN_10(m, p##3) KILN_10(m, p##4) KILN_10(m, p##5) \
                       KILN_10(m, p##6) KILN_10(m, p##7) KILN_10(m, p##8) \
                       KILN_10(m, p##9)
#define KILN_1000 KILN_100(KILN_USE, 0) KILN_100(KILN_USE, 1) \
  KILN_100(KILN_USE, 2) KILN_100(KILN_USE, 3) KILN_100(KILN_USE, 4) \
  KILN_100(KILN_USE, 5) KILN_100(KILN_USE, 6) KILN_100(KILN_USE, 7) \
  KILN_100(KILN_USE, 8) KILN_100(KILN_USE, 9)
int use(const P49 &p) {
  int r = 0;
  KILN_1000 KILN_1000 KILN_1000 KILN_1000 KILN_1000 KILN_1000
  return r;
}

// The chain is resolved, so a compiler that skips it cannot look fast.
static_assert(__is_same(decltype(P49{}.operator->()), P48));
static_assert(__is_same(decltype(P49{}->m), int));
