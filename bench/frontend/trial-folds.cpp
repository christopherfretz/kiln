// Trial folding of ordinary runtime code (WP-656): no reflection, no
// constexpr functions. 4,000 blocks, each an enumeration with computed
// enumerators, a struct with bit-field widths and array bounds from integer
// arithmetic and sizeof/alignof, `static const` and `const` integer
// variables, and a function whose brace initializations are checked for
// narrowing, whose `if`/`while` conditions are constant and whose
// `switch`es have computed `case` labels and enumerators. Every such
// expression is folded speculatively (sema's `try_fold`/`fold_int`) or as an
// integral constant expression, so this measures the per-fold cost of the
// constant evaluator's entry on the shapes ordinary code is made of.

#define KILN_D1(m, p) m(p##0) m(p##1) m(p##2) m(p##3) m(p##4) \
  m(p##5) m(p##6) m(p##7) m(p##8) m(p##9)
#define KILN_D2(m, p) KILN_D1(m, p##0) KILN_D1(m, p##1) KILN_D1(m, p##2) \
  KILN_D1(m, p##3) KILN_D1(m, p##4) KILN_D1(m, p##5) KILN_D1(m, p##6)     \
  KILN_D1(m, p##7) KILN_D1(m, p##8) KILN_D1(m, p##9)
#define KILN_D3(m, p) KILN_D2(m, p##0) KILN_D2(m, p##1) KILN_D2(m, p##2) \
  KILN_D2(m, p##3) KILN_D2(m, p##4) KILN_D2(m, p##5) KILN_D2(m, p##6)     \
  KILN_D2(m, p##7) KILN_D2(m, p##8) KILN_D2(m, p##9)

#define KILN_BLOCK(n)                                                        \
  enum E##n { A##n = n % 61, B##n = A##n + 3, C##n = (B##n << 2) | 1,          \
              D##n = sizeof(long) * 8 - 1, F##n = -(n % 13) - 200 };          \
  struct S##n {                                                              \
    unsigned a : 3;                                                          \
    unsigned b : (n % 5) + 1;                                                \
    int arr[(n % 7) + 4 * 2];                                                \
    char tag[sizeof(int) + alignof(double)];                                 \
    short pad[16 / 2 - 1];                                                   \
  };                                                                         \
  static const int k##n = n * 3 + 7;                                         \
  const long l##n = (n << 3) - 1;                                            \
  static const unsigned m##n = 0xFFFFu & ~0x0Fu;                             \
  int f##n(int x, E##n e) {                                                  \
    short s{100 + n % 50};                                                   \
    unsigned char uc{200};                                                   \
    long buf[16 / 2 + 1] = {1, 2, 3};                                        \
    char ch{'a' + 1};                                                        \
    int sum = 0;                                                             \
    if (sizeof(void *) == 8 && 1 + 1 == 2) sum += 3;                         \
    if (n % 2 == 0) sum -= 1;                                                \
    while (0 && x) sum++;                                                    \
    switch (x) {                                                             \
    case 0: sum += 1; break;                                                 \
    case 1 + 1: sum += 2; break;                                             \
    case n % 13 + 10: sum += 3; break;                                       \
    case (1 << 5) - 1: sum += 4; break;                                      \
    case 'A' * 2: sum += 5; break;                                           \
    default: break;                                                          \
    }                                                                        \
    switch (e) {                                                             \
    case A##n: sum += 6; break;                                              \
    case C##n: sum += 7; break;                                              \
    case D##n: sum += 8; break;                                              \
    case F##n: sum += 9; break;                                              \
    default: break;                                                          \
    }                                                                        \
    unsigned u = 0xFFu >> 2;                                                 \
    int neg = -(n % 9) * 4;                                                  \
    S##n st{1, 1, {2, 3}, {4}, {5}};                                         \
    return sum + s + uc + buf[2] + ch + k##n + int(l##n) + int(u + m##n) +   \
           neg + st.arr[1];                                                  \
  }

KILN_D3(KILN_BLOCK, 1)
KILN_D3(KILN_BLOCK, 2)
KILN_D3(KILN_BLOCK, 3)
KILN_D3(KILN_BLOCK, 4)

static_assert(k4999 == 4999 * 3 + 7);
static_assert(C4999 == (((4999 % 61) + 3) << 2 | 1));
