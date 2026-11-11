/* The C side of cxx-calls-c.cpp: unmangled names, and identifiers that
   are keywords in C++. */
int c_calls = 0;
int c_sum(const int *v, int n) {
  int new = 0, this;
  for (this = 0; this < n; ++this)
    new += v[this];
  ++c_calls;
  return new;
}
