// [temp.type]/2.4, [temp.arg.nontype]: a pointer or reference template
// argument designating a union member is printed with that member
// (`A<&u.b>`, not the union's first member of the same type, which kiln
// printed before WP-2200), and substituted into the template it reads
// that member: `*P` with `P = &cu.a` while `b` is active is a read of
// an inactive member ([expr.const]/10.9). Reported by a downstream user.
//@ verify
//@ std: c++26
template <auto V> struct A {};
template <auto &R> struct Ref {};
union U { int a; int b; } u;
struct S1 { int x; };
struct S2 { int x; };
union V { S1 s; S2 t; } v;
union W { int arr[2]; int b; } w;
union N { U p; U q; } n;
int i1 = A<&u.b>::x;      // expected-error {{no member named 'x' in 'A<&u.b>'}}
int i2 = A<&v.t.x>::x;    // expected-error {{no member named 'x' in 'A<&v.t.x>'}}
int i3 = A<&v.t>::x;      // expected-error {{no member named 'x' in 'A<&v.t>'}}
int i4 = A<&w.b>::x;      // expected-error {{no member named 'x' in 'A<&w.b>'}}
int i5 = A<&w.arr[1]>::x; // expected-error {{no member named 'x' in 'A<&w.arr[1]>'}}
int i6 = Ref<u.b>::x;     // expected-error {{no member named 'x' in 'Ref<u.b>'}}
int i7 = A<&n.q.b>::x;    // expected-error {{no member named 'x' in 'A<&n.q.b>'}}
int i8 = A<&n.q.a>::x;    // expected-error {{no member named 'x' in 'A<&n.q.a>'}}
int i9 = A<&n.p.b>::x;    // expected-error {{no member named 'x' in 'A<&n.p.b>'}}
constexpr int *pb = &u.b;
int i10 = A<pb>::x;       // expected-error {{no member named 'x' in 'A<&u.b>'}}

constexpr union U2 { int a; int b; } cu{.b = 5};
template <const int *P> constexpr int read() { return *P; } // expected-note {{read of member 'a' of union with active member 'b' is not allowed in a constant expression}}
static_assert(read<&cu.b>() == 5);
static_assert(read<&cu.a>() == 5); // expected-error {{static assertion expression is not an integral constant expression}} \
                                   // expected-note {{in call to 'read<&cu.a>()'}}
