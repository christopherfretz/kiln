// Placement and nothrow allocation functions ([new.delete.placement],
// [new.delete.single]): null checks, user operators, size_t-typed sizes.
namespace std { struct nothrow_t { }; extern const nothrow_t nothrow; enum class align_val_t : unsigned long {}; }
void *operator new(unsigned long, const std::nothrow_t &) noexcept;
void *operator new(unsigned long, int);
void *operator new[](unsigned long, int, int) noexcept;
inline void *operator new[](unsigned long, void *p) noexcept { return p; }
int *a() { return new (std::nothrow) int(4); }
int *b() { return new (std::nothrow) int; }
int *c() { return new (7) int; }
int *d(int n) { return new (1, 2) int[n]{3}; }
long double *f() { return new long double(1); }
struct S3 { int a, b, c; };
S3 *g() { return new S3{1, 2, 3}; }
char *h(int n) { return new char[n]; }
short *i(long long n) { return new short[n]; }
int *j(unsigned n) { return new int[n]{1}; }
char *k(unsigned __int128 n) { return new char[n]; }
