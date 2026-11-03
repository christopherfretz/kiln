// WP-416: __builtin_align_up / __builtin_align_down / __builtin_is_aligned
// (clang `EmitBuiltinAlignTo`), __builtin_debugtrap,
// __builtin_frame_address and __builtin_elementwise_abs / _max / _min.
// Calls that fold at clang 19 and 22 alike (the alignment builtins on
// integers) are folded; the elementwise folds are clang 22's only and
// live in builtins_elementwise_22.cpp.

unsigned char *ad(unsigned char *p) { return __builtin_align_down(p, 16); }
unsigned char *au(unsigned char *p) { return __builtin_align_up(p, 16); }
bool ia(unsigned char *p) { return __builtin_is_aligned(p, 16); }
unsigned ad2(unsigned x, unsigned a) { return __builtin_align_down(x, a); }
unsigned au2(unsigned x) { return __builtin_align_up(x, 8); }
bool ia2(int x) { return __builtin_is_aligned(x, 4); }
unsigned long long au3(unsigned long long x, int a) { return __builtin_align_up(x, a); }
short ad3(short x, long long a) { return __builtin_align_down(x, a); }
bool ia3(void *p, unsigned a) { return __builtin_is_aligned(p, a); }
unsigned char *au4(unsigned char *p, int a) { return __builtin_align_up(p, a); }
int *ad4(int *p, long long a) { return __builtin_align_down(p, a); }
int c1() { return __builtin_align_up(13, 8); }
bool c2() { return __builtin_is_aligned(16, 8); }

void dt() { __builtin_debugtrap(); }
void *fa() { return __builtin_frame_address(0); }

int eabs(int x) { return __builtin_elementwise_abs(x); }
float eabsf(float x) { return __builtin_elementwise_abs(x); }
int emax(int x, int y) { return __builtin_elementwise_max(x, y); }
unsigned emin(unsigned x, unsigned y) { return __builtin_elementwise_min(x, y); }
double emaxd(double x, double y) { return __builtin_elementwise_max(x, y); }
float eminf(float x, float y) { return __builtin_elementwise_min(x, y); }
int e2(int x) { return __builtin_elementwise_max(x, 3); }
long long eabsl(long long x) { return __builtin_elementwise_abs(x); }

typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned v4u __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
v4i vabs(v4i x) { return __builtin_elementwise_abs(x); }
v4i vmax(v4i x, v4i y) { return __builtin_elementwise_max(x, y); }
v4u vminu(v4u x, v4u y) { return __builtin_elementwise_min(x, y); }
v4f vabsf(v4f x) { return __builtin_elementwise_abs(x); }
v4f vmaxf(v4f x, v4f y) { return __builtin_elementwise_max(x, y); }
