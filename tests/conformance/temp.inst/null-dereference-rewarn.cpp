// [temp.inst], [temp.res.general]/6, [expr.unary.op]/1: -Wnull-dereference
// (clang 19 CheckForNullPointerDereference) in templates. A function
// template's definition warns, unfiltered by reachability; a member of a
// class template, a lambda in a template and a generic lambda do not (clang
// skips analysis-based warnings in a dependent context). Every
// instantiation warns again under its instantiation note, including for a
// non-dependent dereference the instantiation shares with the definition,
// filtered by the instantiation's reachability and dropped in unevaluated
// operands and discarded statements. Division by zero is not repeated.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 60:42 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 60:42 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 61:45 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 62:52 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 62:52 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 62:39 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 62:39 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 63:71 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 63:71 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 64:63 warning "division by zero is undefined"
//@ diag: 65:85 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 65:85 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 69:39 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 69:39 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 72:41 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 72:63 note "in instantiation of function template specialization 'glam()::(anonymous class)::operator()<int>' requested here"
//@ diag: 72:41 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 72:41 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 72:70 note "in instantiation of function template specialization 'glam()::(anonymous class)::operator()<double>' requested here"
//@ diag: 72:41 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 60:42 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 73:9 note "in instantiation of function template specialization 'nondep<int>' requested here"
//@ diag: 60:42 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 60:42 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 73:25 note "in instantiation of function template specialization 'nondep<long>' requested here"
//@ diag: 60:42 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 61:45 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 73:42 note "in instantiation of function template specialization 'bind<int>' requested here"
//@ diag: 62:52 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 73:52 note "in instantiation of function template specialization 'two<int>' requested here"
//@ diag: 62:52 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 62:39 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 62:39 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 63:71 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 74:9 note "in instantiation of function template specialization 'reach<char>' requested here"
//@ diag: 63:71 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 65:85 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 74:54 note "in instantiation of function template specialization 'disc<char>' requested here"
//@ diag: 65:85 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 67:20 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 74:78 note "in instantiation of member function 'C<int>::g' requested here"
//@ diag: 67:20 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 69:39 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 75:33 note "in instantiation of function template specialization 'C<int>::m<int>' requested here"
//@ diag: 69:39 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 71:53 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 75:44 note "in instantiation of function template specialization 'lam<int>' requested here"
//@ diag: 71:53 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
template <class T> int nondep() { return *(int *)0; }
template <class T> int bind(T t) { int &r = *(int *)0; return r + t; }
template <class T> int two() { return *(int *)0 + (*(short *)0 = 1); }
template <class T> int reach() { if (sizeof(T) == 4) return 0; return *(int *)0; }
template <class T> int quiet() { return sizeof(*(int *)0) + 1 / 0; }
template <class T> int disc() { if constexpr (sizeof(T) == 4) return 0; else return *(int *)0; }
template <class T> struct C {
  int g() { return *(int *)0; }
  int h() { int x = 1 / 0; return x; }
  template <class U> int m() { return *(int *)0; }
};
template <class T> int lam() { auto l = [] { return *(int *)0; }; return l(); }
int glam() { auto l = [](auto) { return *(int *)0; }; return l(1) + l(2.0); }
int i = nondep<int>() + nondep<long>() + bind(1) + two<int>() + reach<int>() +
        reach<char>() + quiet<int>() + disc<int>() + disc<char>() + C<int>().g() +
        C<int>().h() + C<int>().m<int>() + lam<int>();
