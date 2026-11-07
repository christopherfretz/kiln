// [class.union.general], [class.default.ctor]/3, [class.dtor]/8 (P3074R7,
// C++26): a union of a string-like class (owning heap storage, non-trivial
// special members) has a trivial default constructor and destructor, which
// emit nothing; the member is constructed and destroyed by hand at run
// time, as std::optional and inplace_vector storage do. The storage of a
// class with an anonymous union of an array of them is per element.
//@ skip(clang): clang 22 does not implement P3074 (trivial unions)
//@ std: c++26
//@ expect: run
//@ strict-diags
//@ stdout: [hello] [world!] [a][bc][def] 3 live=0
extern "C" int printf(const char *, ...);
extern "C" void *malloc(decltype(sizeof 0));
extern "C" void free(void *);
void *operator new(decltype(sizeof 0), void *p) noexcept { return p; }
static int live = 0;
struct Str {
  char *d;
  Str(const char *s) {
    int n = 0;
    while (s[n]) ++n;
    d = static_cast<char *>(malloc(n + 1));
    for (int i = 0; i <= n; ++i) d[i] = s[i];
    ++live;
  }
  ~Str() { free(d); --live; }
};
union U { Str s; int i; };
static_assert(__is_trivially_constructible(U) && __is_trivially_destructible(U));

template <class T, int N> struct IV {
  union { T data[N]; };
  int n = 0;
  IV() {}
  ~IV() { while (n) data[--n].~T(); }
  void push(const char *s) { ::new (&data[n]) T(s); ++n; }
};

int main() {
  U u;
  ::new (&u.s) Str("hello");
  printf("[%s] ", u.s.d);
  u.s.~Str();
  ::new (&u.s) Str("world!");
  printf("[%s] ", u.s.d);
  u.s.~Str();
  u.i = 3;
  {
    IV<Str, 4> v;
    v.push("a");
    v.push("bc");
    v.push("def");
    for (int i = 0; i < v.n; ++i) printf("[%s]", v.data[i].d);
    printf(" %d", u.i);
  }
  printf(" live=%d\n", live);
}
