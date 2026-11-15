// [dcl.array]/1, [intro.memory]: an array type whose size exceeds the
// target's maximum object size (61 address bits on the 64-bit targets) is
// an error at its bound, before anything is laid out (clang
// `BuildArrayType`, "array is too large"): a single bound, the product of
// the bounds of a multidimensional array, an array of class type, a bound
// or an element supplied by a template argument, and a `new` bound. Found
// running GCC's C++ testsuite, where kiln ran out of memory laying out the
// instantiation (WP-1456). `Edge` and `ok` are at the limit. The bound
// of `Many` is reported one column early (at its `[`): column not pinned.
//@ mem<=2048
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:44 error "array is too large ("
//@ diag: 23:13 note "in instantiation of template class 'Cube<66000>' requested here"
//@ diag: 24:9 error "array is too large ("
//@ diag: 26:10 error "array is too large ("
//@ diag: 27 error "array is too large ("
//@ diag: 28:11 note "in instantiation of template class 'Many<Gig>' requested here"
//@ diag: 29:26 error "array is too large ("
//@ diag: 30:18 error "array is too large ("
//@ diag: 33:38 error "array is too large ("
template <int N> struct Cube { int a[N][N][N][N][N][N]; };
Cube<66000> cube;
int big[1LL << 40][1LL << 40];
struct Gig { char c[1LL << 31]; };
Gig gigs[1LL << 31];
template <class T> struct Many { T t[1LL << 40]; };
Many<Gig> many;
void f() { (void)new int[1LL << 40][1LL << 40]; }
typedef int Huge[1LL << 59];
Huge huge;
using Edge = char[(1LL << 61) - 1];
template <class T> struct Fine { T t[1LL << 61]; };
Fine<char> fine;
long long ok[1LL << 57];
