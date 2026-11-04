// throw_no_exceptions.cpp's system header (`#pragma GCC system_header`).
#pragma GCC system_header
struct Thrown {
  Thrown();
  ~Thrown();
  int v;
};
inline void sys_throw(int x) {
  if (x)
    throw x;
}
template <class T> int sys_try_throw(T t) {
  try {
    if (t)
      throw Thrown();
    return 1;
  } catch (const Thrown &e) {
    return e.v;
  } catch (...) {
    throw;
  }
}
