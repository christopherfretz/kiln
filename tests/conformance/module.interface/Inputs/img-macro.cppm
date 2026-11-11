// Interface unit of module `imgm` for image-macro-run.cpp (spec/modules.md
// §11.11 "Expansion records", stage S7): declarations whose tokens come
// from macro expansions, macro arguments, pasting and nested expansions,
// so their locations lie in macro space.
export module imgm;
#define IMGM_ADD(a, b) ((a) + (b))
#define IMGM_FN(name, body)                                                   \
  export constexpr int name(int x) { return body; }
#define IMGM_PAIR(name)                                                       \
  IMGM_FN(name##_lo, IMGM_ADD(x, 1))                                          \
  IMGM_FN(name##_hi, IMGM_ADD(x, 100))
#define IMGM_STRUCT(T, field)                                                 \
  export struct T {                                                           \
    int field;                                                                \
    constexpr int get() const { return field; }                               \
  };
#define IMGM_TPL template <class T> constexpr T
#define IMGM_ENUM(E, ...) export enum class E { __VA_ARGS__ };
IMGM_FN(inc, IMGM_ADD(x, 1))
IMGM_PAIR(edge)
IMGM_STRUCT(Cell, value)
export IMGM_TPL negate(T t) { return -t; }
IMGM_ENUM(Dir, north, east = 4, south)
export constexpr int kSum = IMGM_ADD(inc(1), edge_hi(1));
