// [expr.const]/10.9 and clang's ext_vector_type: `.hi` and `.odd` of a
// vector with an odd number of lanes select a lane past its end (clang
// lays the vector out with the lane count rounded up); that lane holds no
// value, so the access is a constant expression as long as nothing reads
// the lane: it can initialize an object, be discarded, or be narrowed to
// the lanes that exist
//@ expect: compile-ok
//@ strict-diags
typedef int e2i __attribute__((ext_vector_type(2)));
typedef int e3i __attribute__((ext_vector_type(3)));
typedef int e5i __attribute__((ext_vector_type(5)));
constexpr e3i e3 = {1, 2, 3};
constexpr e2i h3 = (e3 + 0).hi;
constexpr e2i o3 = (e3 + 0).odd;
constexpr e3i h5 = (e5i){1, 2, 3, 4, 5}.hi;
constexpr e3i o5 = (e5i){1, 2, 3, 4, 5}.odd;
constexpr e2i l3 = (e3 + 0).lo, v3 = (e3 + 0).even;
static_assert((e3 + 0).hi.x == 3 && (e3 + 0).odd.x == 2);
static_assert((e5i){1, 2, 3, 4, 5}.hi.y == 5 && (e5i){1, 2, 3, 4, 5}.odd.y == 4);
static_assert((e5i){1, 2, 3, 4, 5}.hi.yx.x == 5 && (e5i){1, 2, 3, 4, 5}.hi.x == 4);
static_assert(((l3 + v3) == (e2i){2, 5}).y);
constexpr int discard() {
  (void)(e3 + 0).hi;
  (void)(e3 + 0).odd.x;
  return 1;
}
static_assert(discard() == 1);
