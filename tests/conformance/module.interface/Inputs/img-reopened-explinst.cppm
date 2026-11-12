export module imgrei;
export namespace rei {
template <class T> struct basic {
  T v;
  int width() const { return int(sizeof(T)) * 10 + int(v); }
};
typedef basic<long> wide;
} // namespace rei
export namespace rei {
template struct basic<long>;
template struct basic<short>;
} // namespace rei
