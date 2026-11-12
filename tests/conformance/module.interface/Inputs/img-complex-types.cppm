export module imgcxt;
export namespace cxt {
typedef __complex__ float cfloat;
inline __complex__ float make(float a, float b) {
  __complex__ float z;
  __real__ z = a;
  __imag__ z = b;
  return z;
}
template <class T> struct box {
  typedef _Complex double type;
  type v;
  T real() const { return T(__real__ v); }
  T imag() const { return T(__imag__ v); }
};
inline _Complex double twice(_Complex double z) { return z * 2.0; }
inline float norm(cfloat z) {
  return __real__ z * __real__ z + __imag__ z * __imag__ z;
}
} // namespace cxt
