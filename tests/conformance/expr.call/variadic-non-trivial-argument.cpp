// [expr.call]/12: a class object with a non-trivial copy/move constructor or
// destructor for the ellipsis is conditionally-supported: -Wnon-pod-varargs,
// error by default, not in unevaluated or constant-initializer contexts.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:8 error "cannot pass object of non-trivial type 'NT' through variadic function; call will abort at runtime"
//@ diag: 32:8 error "cannot pass object of non-trivial type 'NT' through variadic function"
//@ diag: 33:8 error "cannot pass object of non-trivial type 'Dtor' through variadic function"
//@ diag: 34:8 error "cannot pass object of non-trivial type 'Move' through variadic function"
//@ diag: 35:12 error "cannot pass object of non-trivial type 'NT' through variadic method"
//@ diag: 38:8 error "cannot pass object of non-trivial type 'NT' through variadic method"
//@ diag: 39:10 error "cannot pass object of non-trivial type 'NT' through variadic constructor"
//@ diag: 48:11 error "cannot pass object of non-trivial type 'NT' through variadic function"
//@ diag: 50:15 note "in instantiation of function template specialization 'tt<int>' requested here"
//@ diag: 47:11 error "cannot pass object of non-trivial type 'NT' through variadic function"
//@ diag: 51:15 note "in instantiation of function template specialization 'tt<NT>' requested here"
//@ diag: 48:11 error "cannot pass object of non-trivial type 'NT' through variadic function"
//@ diag: 52:15 error "cannot pass object of non-trivial type 'NT' through variadic function"
void g(int, ...);
struct NT { NT(); NT(const NT &); ~NT(); };
struct Dtor { ~Dtor(); };
struct Move { Move(); Move(Move &&); Move(const Move &) = default; };
struct Triv { int x; Triv() {} };
struct Agg { int a[2]; };
struct M { void m(int, ...); };
struct C { C(int, ...); };
constexpr int cf(int, ...) { return 1; }

void calls() {
  NT nt;
  g(1, nt);
  g(1, NT());
  g(1, Dtor());
  g(1, Move());
  M().m(1, nt);
  g(1, Triv(), Agg{}, "s", nullptr);
  auto l = [](int, ...) {};
  l(1, nt);
  C c(1, nt);
  (void)sizeof(cf(1, nt));
  (void)noexcept(g(1, nt));
  decltype(cf(1, NT())) d = 0;
  (void)d;
}

template<class T> void tt(T t) {
  g(1, 2, t);
  g(1, 2, NT());
}
template void tt<int>(int);
template void tt<NT>(NT);
int i = (g(1, NT()), 0);
