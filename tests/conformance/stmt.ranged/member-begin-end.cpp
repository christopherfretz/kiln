// [stmt.ranged]/1.2: a class range with member `begin` and `end` uses
// `__range.begin()` and `__range.end()`; a class iterator needs `!=`,
// prefix `++` and unary `*`.
//@ expect: run
//@ stdout: 0 1 2 3 4 | 10
extern "C" int printf(const char*, ...);

struct Iota {
  struct It {
    int v;
    bool operator!=(const It& o) const { return v != o.v; }
    It& operator++() { ++v; return *this; }
    int operator*() const { return v; }
  };
  int n;
  It begin() const { return {0}; }
  It end() const { return {n}; }
};

struct Arr {
  int d[4] = {1, 2, 3, 4};
  int* begin() { return d; }
  int* end() { return d + 4; }
};

int main() {
  for (int i : Iota{5}) printf("%d ", i);
  Arr a;
  int s = 0;
  for (int& x : a) s += x;
  printf("| %d\n", s);
}
