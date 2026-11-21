// Thread-local variables of pch-thread-local-run.cpp, which a
// precompiled header's snapshot carries (WP-1824).
extern "C" int printf(const char *, ...);
namespace ptl {
inline int trace(int v) { return v; }
thread_local int k = 3;
thread_local int d = trace(4);
extern thread_local int ext;
__thread int st = 5;
_Thread_local int c11 = 6;
[[gnu::tls_model("initial-exec")]] thread_local int ie = trace(7);
constinit thread_local int ci = 8;
constexpr thread_local int cx = 9;
thread_local const int cc = trace(10);
thread_local int arr[3] = {1, trace(11), 3};
thread_local int &ref = arr[1];
static thread_local int internal = trace(12);
inline thread_local int inl = trace(13);
struct S {
  static thread_local int sm;
  static inline thread_local int si = trace(14);
};
thread_local int S::sm = trace(15);
struct T {
  int v;
  T() : v(trace(16)) {}
  ~T() { printf("~T %d\n", v); }
};
thread_local T t;
template <class U> struct Box {
  static thread_local U v;
};
template <class U> thread_local U Box<U>::v = U(trace(17));
template <class U> thread_local U vt = U(trace(18));
template <class U> U bump() {
  static thread_local U n{};
  return ++n;
}
inline int counter() {
  static thread_local int n = 0;
  return ++n;
}
} // namespace ptl
