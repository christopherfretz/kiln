// clang's `annotate` attribute on non-static data members (WP-1113,
// [dcl.attr.grammar]/5, [class.mem]/1): every access to the member wraps
// its address in one `@llvm.ptr.annotation` call per attribute (after the
// GEP, after loading a reference member's bound address, on the object's
// own address in a union), in a constructor's member initializer too; a
// bit-field gets none, and neither does binding a reference member.
struct S { int m [[clang::annotate("field", 3)]]; int n [[clang::annotate("f2")]] [[clang::annotate("f3")]]; };
int h(S& s) { s.n = 1; return s.m; }
struct R { int& r [[clang::annotate("ref")]]; int b [[clang::annotate("bf")]] : 3; };
union U { int i [[clang::annotate("ui")]]; float f; };
int useR(R& r, U& u) { r.b = 1; u.i = 2; return r.r; }
struct C {
  int a [[clang::annotate("ca")]];
  int c = 4;
  C() : a(1) {}
  C(int x) : a(x), c(a) {}
};
struct Agg { int x [[clang::annotate("ax")]]; int y; };
int useAgg(int v) {
  Agg a{1, 2};
  Agg b = {v, 3};
  C c;
  C d(2);
  return a.x + b.y + c.a + d.c;
}
struct Ref { int& r [[clang::annotate("rb")]]; };
int bind(int i) { Ref x{i}; return x.r; }
