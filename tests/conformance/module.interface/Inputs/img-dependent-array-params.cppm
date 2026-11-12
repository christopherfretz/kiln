export module imgdap;
export namespace dap {
template <class T> T first2(const T a[2]) { return a[0] + a[1]; }
template <class T> T head(T a[]) { return a[0]; }
template <class T> struct holder {
  T v;
  template <class U> T scaled(const U k[3]) const { return v * T(k[2]); }
};
} // namespace dap
