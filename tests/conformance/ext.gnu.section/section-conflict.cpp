// GCC section (clang `Sema::UnifySection`): each declaration placed in a
// named section gives it flags, a function read and execute, a variable
// read, and write too unless its storage is constant (a `const` or
// reference type, no `mutable` member, a trivial destructor, a constant
// initializer). A declaration whose flags differ from the section's first
// declaration's is an error with a note at that first declaration. A
// function is placed at each declaration that carries the attribute, a
// variable at its definition (an inherited attribute counts), a template
// pattern but not an instantiation. kiln accepted all of these.
//@ verify
//@ std: c++20

namespace N { const int a __attribute__((section("s1"))) = 1; } // expected-note 5 {{declared here}}
int b __attribute__((section("s1"))) = 2; // expected-error {{'b' causes a section type conflict with 'a'}}
void f() __attribute__((section("s2"))); // expected-note 2 {{declared here}}
void f() {}
int c __attribute__((section("s2"))); // expected-error {{'c' causes a section type conflict with 'f'}}
extern int d __attribute__((section("s3")));
const int e __attribute__((section("s3"))) = 3; // expected-note {{declared here}}
int g __attribute__((section("s3"))); // expected-error {{'g' causes a section type conflict with 'e'}}
void h() { static int sl __attribute__((section("s1"))) = 1; } // expected-error {{'sl' causes a section type conflict with 'a'}}
int &r __attribute__((section("s1"))) = b;
struct M { mutable int m; };
const M cm __attribute__((section("s1"))) = {}; // expected-error {{'cm' causes a section type conflict with 'a'}}
struct D { ~D(); };
const D cd __attribute__((section("s1"))) = {}; // expected-error {{'cd' causes a section type conflict with 'a'}}
struct Ct { Ct(); };
const Ct cc __attribute__((section("s1"))); // expected-error {{'cc' causes a section type conflict with 'a'}}
struct K { constexpr K() {} int x = 0; };
const K ck __attribute__((section("s1")));
template <class T> struct TS { static int t; };
template <class T> int TS<T>::t __attribute__((section("s2"))) = 0; // expected-error {{'t' causes a section type conflict with 'f'}}
int u = TS<int>::t;
const int ci __attribute__((section("s4"))) = b;
int nc __attribute__((section("s4")));
void z() __attribute__((section("s5")));
void z2() __attribute__((section("s5")));
[[gnu::section("s6")]] int x1 = 1; // expected-note {{declared here}}
[[gnu::section("s6")]] void x2(); // expected-error {{'x2' causes a section type conflict with 'x1'}}

template <class T> void tf() __attribute__((section("t1"))); // expected-note {{declared here}}
template <class T> void tf() {}
template void tf<int>();
template <class T> T vt __attribute__((section("t1"))) = T(); // expected-error {{'vt' causes a section type conflict with 'tf'}}
int use = vt<int>;
template <class T> struct S { static const int m; void f() __attribute__((section("t2"))); }; // expected-note {{declared here}}
template <class T> const int S<T>::m __attribute__((section("t2"))) = 1; // expected-error {{'m' causes a section type conflict with 'f'}}
int k = S<int>::m;
extern const int ed __attribute__((section("t3")));
const int ed = 1; // expected-note {{declared here}}
int w __attribute__((section("t3"))) = 1; // expected-error {{'w' causes a section type conflict with 'ed'}}
inline int iv __attribute__((section("t4"))) = 1; // expected-note {{declared here}}
constexpr int cx __attribute__((section("t4"))) = 2; // expected-error {{'cx' causes a section type conflict with 'iv'}}
thread_local int tl __attribute__((section("t5"))) = 1; // expected-note {{declared here}}
const int ct __attribute__((section("t5"))) = 1; // expected-error {{'ct' causes a section type conflict with 'tl'}}
int ar[2] __attribute__((section("t7"))); // expected-note {{declared here}}
const int car[2] __attribute__((section("t7"))) = {1, 2}; // expected-error {{'car' causes a section type conflict with 'ar'}}
extern "C" void ef() __attribute__((section("t8"))); // expected-note {{declared here}}
static const char *const sp __attribute__((section("t8"))) = "x"; // expected-error {{'sp' causes a section type conflict with 'ef'}}
