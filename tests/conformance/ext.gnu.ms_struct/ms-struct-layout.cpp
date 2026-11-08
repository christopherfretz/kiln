// GNU `ms_struct` and `#pragma ms_struct` (WP-809; clang `MSStructAttr`,
// `ItaniumRecordLayoutBuilder::LayoutBitField` with `IsMsStruct`): a run
// of bit-fields whose types have one size shares a unit of that size,
// aligned to it; a type of another size, or a bit-field that does not
// fit, starts a new unit; a zero-width bit-field ends the run and aligns
// only after a bit-field; `packed` does not move bit-fields, `aligned`
// and `#pragma pack` only where a unit starts; a union's bit-field
// occupies its whole unit with alignment 1.
//@ expect: layout
#define MS __attribute__((ms_struct))
struct MS S1 { char a : 3; char b : 4; int c : 5; char d : 2; };
struct MS S2 { char a : 3; int : 0; char b : 2; };
struct MS S3 { int a; int : 0; char b; };
struct MS S4 { char a : 3; char : 0; char b : 2; };
struct MS S5 { int : 0; char c; };
struct MS S6 { int a : 3; unsigned b : 4; long long c : 7; unsigned long long d : 60; };
struct MS S7 { short a : 10; short b : 10; short c : 12; };
struct MS S8 { char a; long long b : 3; char c; };
struct __attribute__((packed, ms_struct)) S9 { char a; int b : 4; char c; int d; };
struct MS S10 { char a; int b : 4 __attribute__((packed)); char c; int d __attribute__((packed)); };
#pragma pack(push, 1)
struct MS S11 { char a; int b : 4; short c : 3; char d; };
#pragma pack(2)
struct MS S12 { char a; int b : 4; long long c : 3; char d; };
#pragma pack(pop)
struct MS S13 { char a : 2; int b : 4 __attribute__((aligned(16))); char c; };
union MS U1 { char a : 3; int b : 9; short c : 1; };
union MS U2 { char : 0; char b : 2; };
union MS U3 { int b : 4 __attribute__((aligned(8))); char c; };
enum E : short { e0, e1 };
struct MS S14 { bool a : 1; bool b : 1; E e : 2; short s : 3; bool c : 1; };
struct MS S15 { char a; int : 4; };
struct MS S16 { char a; char b : 4; char c : 4; char d : 1; };
struct MS S17 { char a : 4; unsigned char b : 4; signed char c : 1; };
struct MS S18 { int a : 31; int b : 2; int c : 1; };
struct MS S19 { long long a : 1; int b : 1; long long c : 1; };
struct MS S20 { char a : 1; char : 0; char : 0; int b : 3; char : 0; int c : 2; };
#pragma ms_struct on
struct P1 { char a : 3; int b : 4; };
union P2 { char a : 3; int b : 4; };
#pragma ms_struct off
struct P3 { char a : 3; int b : 4; };
#pragma ms_struct on
#pragma ms_struct reset
struct P4 { char a : 3; int b : 4; };
template <class T> struct MS T1 { char a : 3; T b : 4; };
struct MS S21 { int a : 4; struct In { char c : 2; } in; char d : 3; };
struct MS S22 { char a : 2; int x[2]; char b : 3; };
struct MS S23 { _BitInt(17) a : 3; _BitInt(17) b : 5; char c : 3; };
T1<int> t1; T1<char> t1c;
