// [expr.delete]/10, [new.delete.single] (C++14, N3778): a delete of a
// complete object calls the sized global deallocation function.
// WP-646 feature probe.
//@ expect: run
//@ std: c++14
//@ strict-diags
//@ stdout: 24 1
extern "C" int printf(const char*, ...);
extern "C" void free(void*);
typedef decltype(sizeof 0) size_t;
static size_t seen;
void operator delete(void* p, size_t n) noexcept { seen = n; free(p); }
struct S { long a, b, c; };
int main() {
  S* s = new S;
  delete s;
  printf("%d %d\n", (int)seen, (int)(seen == sizeof(S)));
}
