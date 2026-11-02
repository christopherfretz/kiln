// [expr.prim.this]/3, [class.mem.general]/7, [except.spec]/2: the
// noexcept-specifier of a static member function is a complete-class
// context, but `this` may not appear in it. A non-static member named
// there (called, or as an operand of `sizeof` or `noexcept`) is an implicit
// `this`, diagnosed once per specifier at its first use; static members,
// types and enumerators are fine (clang
// `checkThisInStaticMemberFunctionExceptionSpec`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:37 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 22:35 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 23:37 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 24:37 error "'this' cannot be used in a static member function declaration"
//@ diag: 25:44 error "'this' cannot be implicitly used in a static member function declaration"

struct M {
  void h() noexcept;
};

struct S {
  static void a() noexcept(noexcept(g()));
  static void b() noexcept(sizeof(m) > 0);
  static void c() noexcept(noexcept(m.h()));
  static void d() noexcept(noexcept(this->g()));
  static void e() noexcept(noexcept(s() && g() && g()));
  static void ok1() noexcept(noexcept(s()));
  static void ok2() noexcept(sizeof(T) == 1 && K == 2);
  void ok3() noexcept(noexcept(g()) && sizeof(m) > 0);

  bool g();
  static bool s() noexcept;
  using T = char;
  enum { K = 2 };
  M m;
};

static_assert(noexcept(S::ok1()));
static_assert(noexcept(S::ok2()));
static_assert(!noexcept(S{}.ok3()));
