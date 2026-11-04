// A user header made a system header by `#pragma GCC system_header`: its
// try-blocks are not diagnosed with -fno-exceptions (WP-607).
#pragma GCC system_header
inline int sys_try(int x) {
  try {
    return x;
  } catch (...) {
    return 0;
  }
}
template <class T> T sys_try_t(T t) {
  try {
    return t;
  } catch (const T &) {
    return T();
  }
}
