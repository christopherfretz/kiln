// [over.match.best.general]/2.6, [temp.constr.order]: two operator
// candidates whose constraints are both satisfied but neither subsumes
// the other are ambiguous. WP-1738 (GAP-053).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:34 error "use of overloaded operator '+' is ambiguous (with operand types 'W<long long>' and 'int')"
//@ diag: 15:7 note "candidate function"
//@ diag: 16:7 note "candidate function"
//@ diag: 21:32 error "call to object of type 'W<long long>' is ambiguous"
//@ diag: 17:7 note "candidate function"
//@ diag: 18:7 note "candidate function"
template <class T> concept A = sizeof(T) > 2;
template <class T> concept B = sizeof(T) > 4;
template <class T> struct W {
  int operator+(int) requires A<T>;
  int operator+(int) requires B<T>;
  int operator()() requires A<T>;
  int operator()() requires B<T>;
};
int f(W<long long> w) { return w + 1; }
int g(W<long long> w) { return w(); }
