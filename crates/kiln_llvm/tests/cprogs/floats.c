// Floating-point arithmetic, comparisons and conversions.
double mix(float f, double d, int i, unsigned u) {
  float g = f * 2.0f;
  double r = d / g + i - u;
  if (r < d || r != r)
    return (long)r;
  return -r;
}
