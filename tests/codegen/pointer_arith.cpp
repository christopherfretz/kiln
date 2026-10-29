// Pointer arithmetic, differences, comparisons and indexing.
long pa(int* p, int* q, char* c, double* d, long n) {
  int* r = p + n;
  r = r - 2;
  long diff = q - p;
  char* cc = c + 3;
  ++d;
  d--;
  bool lt = p < q;
  return diff + (r - p) + (cc - c) + lt + (long)(d[1]) + p[n] + *(q - 1);
}
