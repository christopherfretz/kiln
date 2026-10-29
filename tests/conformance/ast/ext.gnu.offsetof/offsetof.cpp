// __builtin_offsetof: a std::size_t prvalue; array indices are children.
struct In { char c; int arr[4]; };
struct S {
  int a;
  double d;
  In in[3];
  struct { short u; short v; };
  union { int w; float f; };
};
constexpr unsigned long od = __builtin_offsetof(S, d);
constexpr unsigned long oi = __builtin_offsetof(S, in[1].arr[2]);
constexpr unsigned long ov = __builtin_offsetof(S, v);
constexpr unsigned long of = __builtin_offsetof(const S, f);
void g() {
  constexpr int k = 1;
  auto x = __builtin_offsetof(S, in[k + 1]);
  (void)x;
}
