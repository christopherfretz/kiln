// Function order and nounwind inference for constexpr function templates
// first instantiated in a constant expression (WP-117 follow-up, WP-204):
// clang instantiates them at their use, so they precede their callers.
struct G { ~G(); };
template<class T> constexpr T sq(T x) { return x * x; }
template<class T> struct W { static constexpr T v = sq(T(2)); T get() const { return sq(v); } };
int f() { G g; W<int> w; return w.get(); }
template<class T> int caller() { G g; return sq(T(3)); }
int u() { return caller<long>(); }
template<class T> constexpr T late(T);
int m() { G g; return late(1); }
template<class T> constexpr T late(T x) { return x; }
constexpr int (*fp)(int) = &late<int>;
