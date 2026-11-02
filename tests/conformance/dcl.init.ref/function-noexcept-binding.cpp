// [dcl.init.ref]/4,5, [conv.fctptr], [over.ics.ref], [over.ics.rank]/3.2: a reference to
// function binds directly to a noexcept function (a function conversion ranked as
// identity, so the reference overloads tie and a reference ties a noexcept pointer).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:8 error "non-const lvalue reference to type 'void (int) noexcept' cannot bind to a value of unrelated type 'void (int)'"
//@ diag: 22:11 error "call to 'g1' is ambiguous"
//@ diag: 20:5 note "candidate function"
//@ diag: 21:6 note "candidate function"
//@ diag: 26:11 error "call to 'g2' is ambiguous"
//@ diag: 24:5 note "candidate function"
//@ diag: 25:6 note "candidate function"
void f(int) noexcept;
void h(int);
void (&r1)(int) = f;
void (&r2)(int) noexcept = f;
void (&r3)(int) noexcept = h;
void (&&r4)(int) = f;
const auto &r5 = f;
int g1(void (&)(int)) { return 1; }
long g1(void (&)(int) noexcept) { return 2; }
auto x1 = g1(f);
auto x2 = g1(h);
int g2(void (&)(int)) { return 1; }
long g2(void (*)(int) noexcept) { return 2; }
auto x3 = g2(f);
int g3(void (*)(int)) { return 1; }
long g3(void (*)(int) noexcept) { return 2; }
auto x4 = g3(f);
int g4(void (&)(int));
auto x5 = g4(f);
template <class T> int g5(T &, void (&)(int));
auto x6 = g5(f, f);
static_assert(__is_same(decltype(x1), long));
static_assert(__is_same(decltype(x4), long));
static_assert(__is_same(decltype(x3), int));
