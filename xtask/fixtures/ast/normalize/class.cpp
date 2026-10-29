// Class features (spec/ast-dump.md §3, WP-066a).
struct B { virtual ~B(); virtual int f() = 0; int b = 1; };
struct V { int v; };
struct F;
void g(int);
class D final : public B, protected virtual V {
  friend struct F;
  friend struct H;
  friend void g(int);
  friend constexpr int k(D) { return 0; }
public:
  mutable int m = 2;
  D() : B(), m(3) {}
  D(int x) : D() {}
  explicit operator bool() const;
  int f() override final { return m; }
  D(const D&) = default;
  D& operator=(const D&) = delete("no \"copy\"");
  static void s();
  ~D();
};
void D::s() {}
struct Q { int q[2]; int z = 1; Q() = default; Q(int, int); };
struct S { S(); S(int, int); ~S(); };
S make();
void use(const S&, int = 5);
int h(D& d, V* w, Q q) {
  B& b = d;
  const S& r = S();
  S&& r2 = make();
  S s1 = S(1, 2);
  S s2{1, 2};
  use(S());
  Q q2 = q;
  V v(1);
  int arr[3](1);
  int i = int(3.0);
  return b.f() + static_cast<B&>(d).b + const_cast<const Q&>(q2).z + i;
}
