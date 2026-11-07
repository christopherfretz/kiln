// An allocation function whose parameters past the placement arguments
// lack default arguments is not viable ([expr.new]/19, [over.match.viable]).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++17
//@ diag: 12:10 error "no matching function for call to 'operator new'"
//@ diag: 9:18 note "candidate function not viable: requires 3 arguments, but 2 were provided"
typedef decltype(sizeof 0) size_t;
struct N { void *operator new(size_t, int, int); };
template <class T> struct M { void *operator new(size_t, T, int = 0); };
void f() {
  N *n = new (1) N;
  M<int> *m = new (1) M<int>;
}
