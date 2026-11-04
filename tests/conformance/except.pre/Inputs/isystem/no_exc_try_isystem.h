// Found through -isystem, so a system header: its try-blocks and
// throw-expressions are not diagnosed with -fno-exceptions (WP-607).
inline int isys_try(int x) {
  try {
    if (x < 0)
      throw x;
    return x;
  } catch (int) {
    return 0;
  }
}
template <class T> struct IsysTry {
  T get(T t) {
    try {
      return t;
    } catch (...) {
      throw;
    }
  }
};
