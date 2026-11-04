// A user header made a system header by `#pragma GCC system_header`: its
// throw-expressions are not diagnosed with -fno-exceptions (WP-607).
#pragma GCC system_header
inline void sys_throw(int x) {
  if (x)
    throw x;
  throw;
}
template <class T> void sys_throw_t(T t) { throw t; }
template <class T> struct SysThrow {
  void m() { throw 1; }
  void n() { throw T(); }
};
