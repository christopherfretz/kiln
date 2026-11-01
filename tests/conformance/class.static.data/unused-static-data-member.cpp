// [class.static.data]/2, [temp.expl.spec] (WP-234): an unused static data
// member of a class with internal linkage is -Wunused-variable (or
// -Wunused-const-variable), at its definition if it has one; a member of
// a class template is not, unless explicitly specialized out of line.
//@ args: -Wunused
//@ strict-diags
//@ diag: 17:8 warning "unused variable 'm'"
//@ diag: 16:43 warning "unused variable 'k'"
//@ diag: 18:8 warning "unused variable 'n'"
//@ diag: 20:8 warning "variable 'q' is not needed and will not be emitted"
//@ diag: 16:116 warning "unused variable 'c'"
//@ diag: 25:27 warning "unused variable 'm'"
//@ diag: 26:27 warning "unused variable 'q'"
//@ diag: 28:16 warning "unused variable 'im'"
namespace {
struct S { static int m; static const int k = 1; static int n; static int used; static int q; static constexpr int c = 4; };
int S::m = 1;
int S::n;
int S::used = 2;
int S::q;
template <class T> struct TS { static int m; static int q; static int r; };
template <class T> int TS<T>::m = 1;
template <class T> int TS<T>::q = 1;
template <class T> int TS<T>::r = 1;
template <> int TS<char>::m = 2;
template <> int TS<long>::q;
struct Outer { struct In { static int im; }; };
int Outer::In::im;
}
int us() { S s; return S::used + TS<int>::m + TS<short>::r + sizeof(s.q); }
struct E { static int sm; };
int E::sm;
