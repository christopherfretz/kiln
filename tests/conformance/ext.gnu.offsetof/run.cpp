// __builtin_offsetof values agree with the record layout, in constant
// evaluation and at run time; members of anonymous structs and unions,
// nested designators and array indices.
//@ expect: run
//@ stdout: 0 8 16 60 76 78 80 80 24 24
extern "C" int printf(const char*, ...);

struct In { char c; int arr[4]; };
struct S {
  int a;
  double d;
  In in[3];
  struct { short u; short v; };
  union { int w; float f; };
};
struct alignas(8) Pad { char c; };
struct Grid { Pad p[2][2]; long tail; };

static_assert(__builtin_offsetof(S, d) == 8);
static_assert(__builtin_offsetof(S, in[1].arr[2]) == 16 + 20 + 4 + 8);
static_assert(__builtin_offsetof(S, f) == __builtin_offsetof(S, w));
static_assert(__builtin_offsetof(Grid, p[1][1]) == 24);
static_assert(__builtin_offsetof(Grid, tail) == sizeof(Pad) * 4);

constexpr unsigned long ix = 2;
int main() {
  unsigned long v[] = {
    __builtin_offsetof(S, a),
    __builtin_offsetof(S, d),
    __builtin_offsetof(S, in),
    __builtin_offsetof(S, in[ix].arr[0]),
    __builtin_offsetof(S, u),
    __builtin_offsetof(S, v),
    __builtin_offsetof(S, w),
    __builtin_offsetof(S, f),
    __builtin_offsetof(Grid, p[1][1]),
    __builtin_offsetof(Grid, p[0][3]),
  };
  printf("%lu %lu %lu %lu %lu %lu %lu %lu %lu %lu\n", v[0], v[1], v[2], v[3], v[4], v[5], v[6],
         v[7], v[8], v[9]);
  return 0;
}
