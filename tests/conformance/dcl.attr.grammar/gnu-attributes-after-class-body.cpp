// [class.pre]/1, [dcl.attr.grammar]/5 (GNU extension): GNU attributes right
// after the closing brace of a class definition appertain to the class, as
// clang's ParseCXXMemberSpecification: `packed` and `aligned` there shape
// the layout before the class is complete (also for a class template, a
// nested class and a union). Elsewhere they stay ill-placed: after a
// declarator or on a non-class type (`'packed' attribute ignored`), on a
// class declaration without a body (`place it after "struct"`).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 32:18 warning "'packed' attribute ignored"
//@ diag: 33:46 warning "'packed' attribute ignored"
//@ diag: 35:25 warning "attribute 'packed' is ignored, place it after \"struct\" to apply attribute to type declaration"
//@ diag: 36:20 warning "'packed' attribute ignored"
struct A { char c; int i; } __attribute__((packed));
static_assert(sizeof(A) == 5 && alignof(A) == 1);
struct C { char c; int i; } __attribute__((packed)) cv;
static_assert(sizeof(cv) == 5);
struct D { char c; int i; } __attribute__((aligned(8)));
static_assert(sizeof(D) == 8 && alignof(D) == 8);
typedef struct G { char c; int i; } __attribute__((packed)) GT;
static_assert(sizeof(GT) == 5);
template <class T> struct TP { char c; T v; } __attribute__((packed));
static_assert(sizeof(TP<int>) == 5 && alignof(TP<int>) == 1);
struct O { struct In { char c; long long l; } __attribute__((packed)) in; char d; };
static_assert(sizeof(O::In) == 9 && sizeof(O) == 10);
union U { char c; int i; } __attribute__((packed, aligned(2)));
static_assert(sizeof(U) == 4 && alignof(U) == 2);
struct __attribute__((aligned(16))) Q { char c; } __attribute__((packed));
static_assert(alignof(Q) == 16);
struct E { char c; int i; };
static_assert(sizeof(E) == 8);
E __attribute__((packed)) e1;
struct F { char c; int i; } x __attribute__((packed));
struct H;
struct H __attribute__((packed));
int __attribute__((packed)) ip;
