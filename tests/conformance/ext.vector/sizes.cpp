// sizeof/alignof of ext_vector types (clang 19 getTypeInfo): a bool
// vector packs one bit per element, rounded up to at least a byte and to a
// power of two; other vectors are N elements wide, rounded up to a power
// of two (WP-151; libc++ simd_utils.h __find_first_set's mask vector).
template <class T, unsigned N> using vec __attribute__((ext_vector_type(N))) = T;
static_assert(sizeof(vec<bool, 1>) == 1 && alignof(vec<bool, 1>) == 1);
static_assert(sizeof(vec<bool, 2>) == 1 && alignof(vec<bool, 2>) == 1);
static_assert(sizeof(vec<bool, 3>) == 1 && alignof(vec<bool, 3>) == 1);
static_assert(sizeof(vec<bool, 4>) == 1 && alignof(vec<bool, 4>) == 1);
static_assert(sizeof(vec<bool, 8>) == 1 && alignof(vec<bool, 8>) == 1);
static_assert(sizeof(vec<bool, 12>) == 2 && alignof(vec<bool, 12>) == 2);
static_assert(sizeof(vec<bool, 16>) == 2 && alignof(vec<bool, 16>) == 2);
static_assert(sizeof(vec<bool, 32>) == 4 && alignof(vec<bool, 32>) == 4);
static_assert(sizeof(vec<bool, 64>) == 8 && alignof(vec<bool, 64>) == 8);
static_assert(sizeof(vec<char, 1>) == 1 && alignof(vec<char, 1>) == 1);
static_assert(sizeof(vec<char, 2>) == 2 && alignof(vec<char, 2>) == 2);
static_assert(sizeof(vec<char, 3>) == 4 && alignof(vec<char, 3>) == 4);
static_assert(sizeof(vec<char, 4>) == 4 && alignof(vec<char, 4>) == 4);
static_assert(sizeof(vec<char, 8>) == 8 && alignof(vec<char, 8>) == 8);
static_assert(sizeof(vec<char, 12>) == 16 && alignof(vec<char, 12>) == 16);
static_assert(sizeof(vec<char, 16>) == 16 && alignof(vec<char, 16>) == 16);
static_assert(sizeof(vec<char, 32>) == 32 && alignof(vec<char, 32>) == 16);
static_assert(sizeof(vec<int, 1>) == 4 && alignof(vec<int, 1>) == 4);
static_assert(sizeof(vec<int, 2>) == 8 && alignof(vec<int, 2>) == 8);
static_assert(sizeof(vec<int, 3>) == 16 && alignof(vec<int, 3>) == 16);
static_assert(sizeof(vec<int, 4>) == 16 && alignof(vec<int, 4>) == 16);
static_assert(sizeof(vec<int, 8>) == 32 && alignof(vec<int, 8>) == 16);
static_assert(sizeof(vec<int, 16>) == 64 && alignof(vec<int, 16>) == 16);
static_assert(sizeof(vec<double, 1>) == 8 && alignof(vec<double, 1>) == 8);
static_assert(sizeof(vec<double, 2>) == 16 && alignof(vec<double, 2>) == 16);
static_assert(sizeof(vec<double, 3>) == 32 && alignof(vec<double, 3>) == 16);
static_assert(sizeof(vec<double, 4>) == 32 && alignof(vec<double, 4>) == 16);
static_assert(sizeof(vec<double, 8>) == 64 && alignof(vec<double, 8>) == 16);
