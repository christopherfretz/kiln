//@ args: -fconstexpr-steps=900000000
// Oven wide-class reads (WP-642, F23 of docs/status/perf-reflection.md):
// the companion of wide-struct-store.cpp without unions. One class with
// 2^WIDE_LOG2 members (4,096 here) of a small class type is stored once,
// member by member, and then read member by member, 1,024 times over, in a constexpr loop.
// No allocation holds a union, so no access walks the class's members on
// the way to the one it touches: this shape is linear in the member count
// when the store shape's union-activity path is the quadratic one. The
// scaling sweep takes -DWIDE_LOG2=<k>.

#ifndef WIDE_LOG2
#define WIDE_LOG2 12
#endif
#ifndef WIDE_ROUNDS
#define WIDE_ROUNDS 1024
#endif

struct M {
  int i;
  short h;
};

// Declares, stores and reads 2^k members named m<binary digits>.
#define D0(p) M p;
#define S0(p) w.p.i = v++;
#define R0(p) s += w.p.i + w.p.h;
#define D1(p) D0(p##0) D0(p##1)
#define D2(p) D1(p##0) D1(p##1)
#define D3(p) D2(p##0) D2(p##1)
#define D4(p) D3(p##0) D3(p##1)
#define D5(p) D4(p##0) D4(p##1)
#define D6(p) D5(p##0) D5(p##1)
#define D7(p) D6(p##0) D6(p##1)
#define D8(p) D7(p##0) D7(p##1)
#define D9(p) D8(p##0) D8(p##1)
#define D10(p) D9(p##0) D9(p##1)
#define D11(p) D10(p##0) D10(p##1)
#define D12(p) D11(p##0) D11(p##1)
#define D13(p) D12(p##0) D12(p##1)
#define D14(p) D13(p##0) D13(p##1)
#define S1(p) S0(p##0) S0(p##1)
#define S2(p) S1(p##0) S1(p##1)
#define S3(p) S2(p##0) S2(p##1)
#define S4(p) S3(p##0) S3(p##1)
#define S5(p) S4(p##0) S4(p##1)
#define S6(p) S5(p##0) S5(p##1)
#define S7(p) S6(p##0) S6(p##1)
#define S8(p) S7(p##0) S7(p##1)
#define S9(p) S8(p##0) S8(p##1)
#define S10(p) S9(p##0) S9(p##1)
#define S11(p) S10(p##0) S10(p##1)
#define S12(p) S11(p##0) S11(p##1)
#define S13(p) S12(p##0) S12(p##1)
#define S14(p) S13(p##0) S13(p##1)
#define R1(p) R0(p##0) R0(p##1)
#define R2(p) R1(p##0) R1(p##1)
#define R3(p) R2(p##0) R2(p##1)
#define R4(p) R3(p##0) R3(p##1)
#define R5(p) R4(p##0) R4(p##1)
#define R6(p) R5(p##0) R5(p##1)
#define R7(p) R6(p##0) R6(p##1)
#define R8(p) R7(p##0) R7(p##1)
#define R9(p) R8(p##0) R8(p##1)
#define R10(p) R9(p##0) R9(p##1)
#define R11(p) R10(p##0) R10(p##1)
#define R12(p) R11(p##0) R11(p##1)
#define R13(p) R12(p##0) R12(p##1)
#define R14(p) R13(p##0) R13(p##1)
#define CAT(a, b) a##b
#define AT(X, k, p) CAT(X, k)(p)

struct Wide {
  AT(D, WIDE_LOG2, m)
};

constexpr long N = 1L << WIDE_LOG2;

constexpr long run() {
  Wide w{};
  long v = 0;
  AT(S, WIDE_LOG2, m)
  long s = 0;
  for (int r = 0; r < WIDE_ROUNDS; ++r) {
    AT(R, WIDE_LOG2, m)
  }
  return s;
}

static_assert(run() == WIDE_ROUNDS * (N * (N - 1) / 2));
