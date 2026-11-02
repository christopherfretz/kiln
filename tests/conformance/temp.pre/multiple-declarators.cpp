// [temp.pre]/5: the init-declarator-list of a template-declaration or explicit
// specialization contains at most one declarator
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:25 error "a template declaration can only declare a single entity"
//@ diag: 23:39 error "a template declaration can only declare a single entity"
//@ diag: 24:26 error "a template declaration can only declare a single entity"
//@ diag: 28:16 error "use of variable template 'b' requires template arguments"
//@ diag: 22:27 note "template is declared here"
//@ diag: 29:9 error "no matching function for call to 'g'"
//@ diag: 24:28 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 31:44 error "a template declaration can only declare a single entity"
//@ diag: 32:30 error "a template declaration can only declare a single entity"
//@ diag: 37:17 error "no matching member function for call to 'k'"
//@ diag: 32:32 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 38:33 error "an explicit template specialization can only declare a single entity"
// The diagnostic is given once, at the first ','; as in clang, every
// declarator is still declared as the first one is: a template with the same
// head, or an explicit specialization.
template <class T>
T v;
template <class T> int a, b, c;
template <class T> constexpr int d = 1, e = 2;
template <class T> T f(T), g();
int x = a<int> + b<long> + c<char>;
static_assert(e<int> == 2);
int y = f(1) + g<int>();
int z = sizeof(b);
int w = g();
struct S {
  template <class T> static const int m = 1, n = 2;
  template <class T> void h(), k();
  int one;
};
static_assert(S::n<int> == 2);
void t(S s) { s.k<int>(); }
void u(S s) { s.k(); }
template <> const int v<int> = 1, v<long> = 2;
static_assert(v<long> == 2);
