// Found through -isystem: a system header (narrowing-in-system-header.cpp).
int sys_narrowed = {2.4};
inline int sys_narrow(double d) {
  int a[] = {d};
  return a[0];
}
template <class T> int sys_tnarrow(T t) { return int{t}; }
struct SysAgg { char c; };
inline SysAgg sys_mk(int i) { return SysAgg{i}; }
