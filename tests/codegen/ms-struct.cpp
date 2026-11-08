// GNU `ms_struct` (WP-809): the discrete bit-field IR (clang
// `isDiscreteBitFieldABI`): each run of bit-fields gets a storage member
// of its first field's type, and a later bit-field joins the run while it
// starts inside that storage; a zero-width bit-field ends the run; a
// union's bit-field storage is its whole type. Loads and stores use the
// run's storage type.
#define MS __attribute__((ms_struct))
struct MS A { char a : 3; char b : 4; int c : 5; char d : 2; };
struct MS B { char a : 3; int : 0; char b : 2; short s : 9; short t : 7; };
struct MS C { long long a : 1; int b : 1; long long c : 63; bool f : 1; };
#pragma ms_struct on
struct P { char x; unsigned u : 31; unsigned v : 1; unsigned w : 1; };
union U { char a : 3; int b : 9; short c : 1; };
#pragma ms_struct off
struct __attribute__((packed, ms_struct)) K { char a; int b : 4; char c; };
int get(A &x) { return x.a + x.b + x.c + x.d; }
void set(A &x) { x.a = 1; x.b = 2; x.c = 3; x.d = 1; }
int get(B &x) { return x.a + x.b + x.s + x.t; }
long long get(C &x) { return x.a + x.b + x.c + x.f; }
unsigned get(P &x) { return x.u + x.v + x.w; }
void set(P &x) { x.w = 1; }
int get(U &x) { return x.a + x.b + x.c; }
void set(U &x) { x.b = 7; }
int get(K &x) { return x.b + x.c; }
