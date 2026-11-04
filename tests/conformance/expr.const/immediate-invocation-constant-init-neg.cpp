// [expr.const.imm]/1.2, [expr.const.defns]/1.6: only the initializer of a
// variable that is usable in constant expressions or has constant
// initialization is manifestly constant-evaluated. Elsewhere (a return
// statement, a constexpr function body, an automatic variable, or a
// static-storage variable whose initializer is not constant) each
// consteval call is an immediate invocation of its own and must be a
// constant expression by itself; its P result points into heap the
// evaluation allocated, so it is not a permitted result ([expr.const]).
// A constexpr or constinit variable whose whole initializer is not
// constant still reports the invocation after its own error
// (spec/sema.md "Immediate invocations"; WP-465, WP-549). Matches clang
// 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 67:16 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 67:16 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
//@ diag: 68:18 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 68:18 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
//@ diag: 69:27 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 69:27 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
//@ diag: 70:20 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 70:20 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
//@ diag: 72:13 error "constexpr variable 'bad1' must be initialized by a constant expression"
//@ diag: 72:21 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 71:5 note "declared here"
//@ diag: 72:25 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 72:25 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
//@ diag: 73:13 error "variable does not have a constant initializer"
//@ diag: 73:1 note "required by 'constinit' specifier here"
//@ diag: 73:21 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 71:5 note "declared here"
//@ diag: 73:25 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 73:25 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
//@ diag: 74:14 error "call to consteval function 'flat' is not a constant expression"
//@ diag: 74:14 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 47:35 note "heap allocation performed here"
struct Buf {
  int* p = nullptr;
  int n = 0;
  constexpr Buf() = default;
  constexpr Buf(const Buf& o) : p(new int[o.n > 0 ? o.n : 1]), n(o.n) {
    for (int i = 0; i < n; ++i) p[i] = o.p[i];
  }
  Buf& operator=(const Buf&) = delete;
  constexpr ~Buf() { delete[] p; }
  constexpr void push(int v) {
    int* q = new int[n + 1];
    for (int i = 0; i < n; ++i) q[i] = p[i];
    q[n++] = v;
    delete[] p;
    p = q;
  }
};
struct D { int a = 0; bool bad = false; };
struct P { Buf entries; D defect{}; };
consteval P flat(int k) {
  P f;
  for (int i = 0; i < k; ++i) f.entries.push(i);
  return f;
}
D g() { return flat(1).defect; }
int h() { return flat(1).entries.p[0]; }
constexpr D cf() { return flat(1).defect; }
void loc() { D l = flat(1).defect; (void)l; }
int nc = 1;
constexpr D bad1 = {nc, flat(1).defect.bad};
constinit D bad2 = {nc, flat(1).defect.bad};
D dyn = {nc, flat(1).defect.bad};
