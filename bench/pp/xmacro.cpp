//@ mode: pp
// X-macro tables, the most common heavy macro idiom in real code bases (LLVM's
// .def files, opcode and diagnostic tables): one table, expanded many times
// with different definitions of the row macro, with token pasting and
// stringizing in every row. The file re-includes itself (16 x 12 passes) so the
// total is well above timer noise. The output is valid C++.
#if !defined(XM_L1)
#define XM_L1

#define XM_CAT(a, b) XM_CAT_I(a, b)
#define XM_CAT_I(a, b) a##b
#define XM_CAT3(a, b, c) XM_CAT(XM_CAT(a, b), c)
#define XM_STR(x) XM_STR_I(x)
#define XM_STR_I(x) #x

// 16 groups x 16 members = 256 rows: X(group, member, opcode, arity, flags).
#define XM_GROUP(X, g, base)                                                       \
  X(g, add, base + 0, 2, XM_F_PURE | XM_F_COMM)                                    \
  X(g, sub, base + 1, 2, XM_F_PURE)                                                \
  X(g, mul, base + 2, 2, XM_F_PURE | XM_F_COMM)                                    \
  X(g, div, base + 3, 2, XM_F_TRAP)                                                \
  X(g, rem, base + 4, 2, XM_F_TRAP)                                                \
  X(g, shl, base + 5, 2, XM_F_PURE)                                                \
  X(g, shr, base + 6, 2, XM_F_PURE)                                                \
  X(g, and, base + 7, 2, XM_F_PURE | XM_F_COMM)                                    \
  X(g, or, base + 8, 2, XM_F_PURE | XM_F_COMM)                                     \
  X(g, xor, base + 9, 2, XM_F_PURE | XM_F_COMM)                                    \
  X(g, neg, base + 10, 1, XM_F_PURE)                                               \
  X(g, not, base + 11, 1, XM_F_PURE)                                               \
  X(g, load, base + 12, 1, XM_F_MEM)                                               \
  X(g, store, base + 13, 2, XM_F_MEM | XM_F_SIDE)                                  \
  X(g, call, base + 14, 3, XM_F_SIDE)                                              \
  X(g, ret, base + 15, 1, XM_F_SIDE)
#define XM_TABLE(X)                                                                \
  XM_GROUP(X, i8, 0)                                                               \
  XM_GROUP(X, i16, 16)                                                             \
  XM_GROUP(X, i32, 32)                                                             \
  XM_GROUP(X, i64, 48)                                                             \
  XM_GROUP(X, u8, 64)                                                              \
  XM_GROUP(X, u16, 80)                                                             \
  XM_GROUP(X, u32, 96)                                                             \
  XM_GROUP(X, u64, 112)                                                            \
  XM_GROUP(X, f16, 128)                                                            \
  XM_GROUP(X, f32, 144)                                                            \
  XM_GROUP(X, f64, 160)                                                            \
  XM_GROUP(X, f128, 176)                                                           \
  XM_GROUP(X, v2, 192)                                                             \
  XM_GROUP(X, v4, 208)                                                             \
  XM_GROUP(X, v8, 224)                                                             \
  XM_GROUP(X, v16, 240)

#define XM_F_PURE 1
#define XM_F_COMM 2
#define XM_F_TRAP 4
#define XM_F_MEM 8
#define XM_F_SIDE 16

#define XM_L2_DIGIT 0
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 1
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 2
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 3
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 4
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 5
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 6
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 7
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 8
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT 9
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT a
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT b
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT c
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT d
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT e
#include "xmacro.cpp"
#undef XM_L2_DIGIT
#define XM_L2_DIGIT f
#include "xmacro.cpp"
#undef XM_L2_DIGIT

int main() { return 0; }

#elif !defined(XM_L2)
#define XM_L2

#define XM_L3_DIGIT 0
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 1
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 2
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 3
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 4
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 5
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 6
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 7
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 8
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT 9
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT a
#include "xmacro.cpp"
#undef XM_L3_DIGIT
#define XM_L3_DIGIT b
#include "xmacro.cpp"
#undef XM_L3_DIGIT

#undef XM_L2
#else
// One pass: a namespace per (L2, L3) digit pair, and the table expanded with
// six different row macros.
#define XM_NS XM_CAT3(t, XM_L2_DIGIT, XM_L3_DIGIT)
namespace XM_NS {
enum class Op : unsigned {
#define XM_ENUM(g, m, op, n, f) XM_CAT3(g, _, m) = (op),
  XM_TABLE(XM_ENUM)
#undef XM_ENUM
};
inline constexpr const char* names[] = {
#define XM_NAME(g, m, op, n, f) XM_STR(XM_NS) "." #g "." #m,
    XM_TABLE(XM_NAME)
#undef XM_NAME
};
inline constexpr unsigned char arity[] = {
#define XM_ARITY(g, m, op, n, f) n,
    XM_TABLE(XM_ARITY)
#undef XM_ARITY
};
inline constexpr unsigned flags[] = {
#define XM_FLAGS(g, m, op, n, f) (f),
    XM_TABLE(XM_FLAGS)
#undef XM_FLAGS
};
inline constexpr bool is_pure(Op o) {
  switch (o) {
#define XM_PURE(g, m, op, n, f)                                                    \
  case Op::XM_CAT3(g, _, m):                                                       \
    return ((f) & XM_F_PURE) != 0;
    XM_TABLE(XM_PURE)
#undef XM_PURE
  }
  return false;
}
struct Counts {
#define XM_COUNT(g, m, op, n, f) unsigned XM_CAT3(m, _, g) = 0;
  XM_TABLE(XM_COUNT)
#undef XM_COUNT
};
}  // namespace XM_NS
#undef XM_NS
#endif
