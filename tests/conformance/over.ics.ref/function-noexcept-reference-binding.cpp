// [dcl.init.ref]/4, /5.1, [over.ics.ref]/1, [over.ics.rank]/3.2: a function
// lvalue of type `void(int) noexcept` binds a reference to `void(int)`
// directly (the types are reference-compatible through the function pointer
// conversion), and that binding is the identity conversion. So `n(g)` with
// candidates taking `void(&)(int) noexcept` and `void(&)(int)` is ambiguous,
// for a function, an overload set, or a function template whose
// specialization is deduced ignoring the exception specification
// ([temp.deduct.funcaddr]). The reverse binding (throwing to noexcept) is
// not reference-compatible. A pointer bound by reference still ranks the
// exact match first.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:8 error "non-const lvalue reference to type 'void (int) noexcept' cannot bind to a value of unrelated type 'void (int)'"
//@ diag: 43:22 error "call to 'n' is ambiguous"
//@ diag: 41:6 note "candidate function"
//@ diag: 42:5 note "candidate function"
//@ diag: 45:22 error "call to 'n' is ambiguous"
//@ diag: 41:6 note "candidate function"
//@ diag: 42:5 note "candidate function"
//@ diag: 46:22 error "call to 'n' is ambiguous"
//@ diag: 41:6 note "candidate function"
//@ diag: 42:5 note "candidate function"
//@ diag: 48:22 error "call to 'n' is ambiguous"
//@ diag: 41:6 note "candidate function"
//@ diag: 42:5 note "candidate function"
//@ diag: 57:22 error "call to 'q' is ambiguous"
//@ diag: 55:6 note "candidate function"
//@ diag: 56:5 note "candidate function"

void g(int) noexcept;
void h(int);
void k(int) noexcept;
void k(long);
template <class T> void f7(T) noexcept;
template <class T> void f8(T);

void (&r1)(int) = g;
void (&&r2)(int) = g;
void (&r3)(int) noexcept = h;

char n(void (&)(int) noexcept);
int n(void (&)(int));
static_assert(sizeof(n(g)) == 1);
static_assert(sizeof(n(h)) == 4);
static_assert(sizeof(n(k)) == 1);
static_assert(sizeof(n(f7)) == 1);
static_assert(sizeof(n(f8)) == 4);
static_assert(sizeof(n(f7<int>)) == 1);

char m(void (*const &)(int) noexcept);
int m(void (*const &)(int));
static_assert(sizeof(m(g)) == 1);
static_assert(sizeof(m(&g)) == 1);

char q(void (&&)(int) noexcept);
int q(void (&&)(int));
static_assert(sizeof(q(g)) == 1);

void one(void (&)(int));
void use() { one(g); one(f7); }
