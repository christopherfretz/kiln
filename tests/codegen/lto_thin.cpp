// [lex.phases]/1.9 (WP-873): `-S -emit-llvm -flto=thin` emits what clang's
// does: the ThinLTO pre-link pipeline (`thinlto-pre-link<O0>`), the
// `EnableSplitLTOUnit` 0 module flag and the module summary. No dynamic
// class here: LLVM 19's IR parser rejects the `typeidCompatibleVTable`
// summary entries its own printer writes for one (lto_thin_split.cpp and
// lto_full.cpp cover the vtable `!type` metadata).
//@ args: -std=c++23 -flto=thin
int counter;
static int bump(int n) { return counter += n; }
inline int twice(int x) { return 2 * x; }
template <class T> T sq(T v) { return v * v; }
struct P { int x, y; int sum() const { return x + y; } };
int use(P p) { return bump(twice(p.sum())) + sq(3) + sq<long>(4); }
extern int ext(int);
int call_ext() { return ext(1); }
