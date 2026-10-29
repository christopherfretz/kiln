// [expr.new]: the reserved placement form `new (p) T` constructs into
// existing storage and returns `p` without allocating.
//@ expect: run
//@ stdout: same 5 | 1 2 3 4
inline void* operator new(unsigned long, void* p) noexcept { return p; }
inline void* operator new[](unsigned long, void* p) noexcept { return p; }
extern "C" int printf(const char*, ...);

struct Pt { int x, y; };

int main() {
  alignas(Pt) unsigned char buf[4 * sizeof(int)];
  int* p = new (buf) int(5);
  printf("%s %d | ", (void*)p == (void*)buf ? "same" : "diff", *p);
  Pt* q = new (buf) Pt[2]{{1, 2}, {3, 4}};
  printf("%d %d %d %d\n", q[0].x, q[0].y, q[1].x, q[1].y);
}
