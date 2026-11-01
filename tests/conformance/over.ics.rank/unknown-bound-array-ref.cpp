// [over.ics.rank]/3.2.6 with P0388R4 reference bindings: binding
// `T (&)[]` and `T (&)[N]` to a `T[N]` lvalue are indistinguishable
// (clang 19 reports the call ambiguous); of two bindings to the same
// array type the less cv-qualified referent wins, an array being
// qualified as its elements are ([basic.type.qualifier]/3)
//@ expect: compile-fail
//@ diag: 23:22 error "call to 'h' is ambiguous"
//@ diag: 21:5 note "candidate function"
//@ diag: 22:6 note "candidate function"
//@ diag: 26:22 error "call to 'h2' is ambiguous"
//@ diag: 24:5 note "candidate function"
//@ diag: 25:6 note "candidate function"
//@ diag: 32:22 error "call to 'h4' is ambiguous"
//@ diag: 30:5 note "candidate function"
//@ diag: 31:6 note "candidate function"



int a[3];
void use();
int h(int (&)[]);
char h(int (&)[3]);
static_assert(sizeof(h(a)) == 1);
int h2(int (&)[]);
char h2(const int (&)[3]);
static_assert(sizeof(h2(a)) == 4);
int h3(int (&)[]);
char h3(const int (&)[]);
static_assert(sizeof(h3(a)) == 4);
int h4(const int (&)[]);
char h4(int (&)[3]);
static_assert(sizeof(h4(a)) == 1);
int h5(int (&)[3]);
char h5(const int (&)[3]);
static_assert(sizeof(h5(a)) == 4);
