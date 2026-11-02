// [over.over]/3, [over.ics.scs]/1, [temp.deduct.funcaddr]/1: the address of
// a function template passed to an overloaded function selects, for each
// candidate, the specialization deduced from that parameter's function type.
// Parameters that differ in noexcept, or in their parameter types, each get
// their own specialization; a dependent noexcept-specifier (left
// uninstantiated by deduction, DR1330) is instantiated before the types are
// compared ([except.spec]/13), and two exact matches are ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:16 error "call to 'pick' is ambiguous"
//@ diag: 20:5 note "candidate function"
//@ diag: 21:6 note "candidate function"
//@ diag: 41:3 error "call to 'q' is ambiguous"
//@ diag: 22:6 note "candidate function"
//@ diag: 23:5 note "candidate function"

int g(int) noexcept;
long g(long);

int pick(int (*)(int) noexcept);
long pick(int (*)(long));
long q(int (*)(long));
int q(int (*)(int));

template <class T> int dep(T v) noexcept(noexcept(g(v))) { return 0; }
template <class T> int nt(T) noexcept { return 0; }
template <class T> int thr(T) { return 0; }

static_assert(sizeof(pick(&nt)) == sizeof(int));
static_assert(sizeof(pick(&thr)) == sizeof(long));
static_assert(sizeof(pick(nt)) == sizeof(int));
static_assert(sizeof(pick(&dep<int>)) == sizeof(int));
static_assert(sizeof(pick(&dep<long>)) == sizeof(long));

int one(int (*)(long));
static_assert(sizeof(one(&thr)) == sizeof(int));

void use() {
  (void)sizeof(pick(&dep));
  pick(&nt);
  q(&thr);
}
