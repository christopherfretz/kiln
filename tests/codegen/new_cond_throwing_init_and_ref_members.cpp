// [expr.new]/17-/28, [except.ctor], [class.mem]: a new-expression in a
// conditional branch whose initializer may throw frees the allocation in
// an EH cleanup guarded by an active flag (clang
// `EnterNewDeleteCleanup` under `isInConditionalBranch`); an explicit
// non-constant `std::align_val_t` placement argument is passed as is; a
// class with a reference member is aggregate-initialized and read
// through the member; a constexpr reference to a lifetime-extended
// temporary is used at run time through its static temporary.
namespace std {
using size_t = decltype(sizeof 0);
enum class align_val_t : size_t {};
}
struct alignas(32) A { int v; };
A *na(std::size_t n) { return new (std::align_val_t(n)) A; }
int f();
struct T { T(int); ~T(); int x; };
T *cn(bool b) { return b ? new T(f()) : nullptr; }
int *ci(bool b) { return b && f() ? new int(f()) : nullptr; }
struct R { int &r; };
int rr(int &x) { R s{x}; return s.r; }
constexpr const int &cr = 5;
const int *pcr() { return &cr; }
struct Q { int a, b; };
constexpr const Q &cq = Q{1, 2};
int qb() { return cq.b; }
