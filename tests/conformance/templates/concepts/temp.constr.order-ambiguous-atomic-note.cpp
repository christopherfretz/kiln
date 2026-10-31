//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:12 error "call to 'f' is ambiguous"
//@ diag: 31:25 note "candidate function [with T = int]"
//@ diag: 32:25 note "candidate function [with T = int]"
//@ diag: 31:40 note "similar constraint expressions not considered equivalent; constraint expressions cannot be considered equivalent unless they originate from the same concept"
//@ diag: 32:40 note "similar constraint expression here"
//@ diag: 39:12 error "call to 'h' is ambiguous"
//@ diag: 37:40 note "candidate function [with T = int]"
//@ diag: 38:40 note "candidate function [with T = int]"
//@ diag: 35:33 note "similar constraint expressions not considered equivalent; constraint expressions cannot be considered equivalent unless they originate from the same concept"
//@ diag: 36:33 note "similar constraint expression here"
//@ diag: 42:6 error "redefinition of 'g'"
//@ diag: 33:6 note "previous definition is here"
//@ diag: 42:12 error "call to 'fa' is ambiguous"
//@ diag: 40:25 note "candidate function [with T = int]"
//@ diag: 41:25 note "candidate function [with T = int]"
//@ diag: 40:41 note "similar constraint expressions not considered equivalent; constraint expressions cannot be considered equivalent unless they originate from the same concept"
//@ diag: 41:41 note "similar constraint expression here"
//@ diag: 45:13 error "call to 'fb' is ambiguous"
//@ diag: 43:25 note "candidate function [with T = int]"
//@ diag: 44:25 note "candidate function [with T = int]"
//@ diag: 43:41 note "similar constraint expressions not considered equivalent; constraint expressions cannot be considered equivalent unless they originate from the same concept"
//@ diag: 44:60 note "similar constraint expression here"
//@ diag: 49:8 error "ambiguous partial specializations of 'S<int>'"
//@ diag: 47:52 note "partial specialization matches [with T = int]"
//@ diag: 48:60 note "partial specialization matches [with T = int]"
// Ambiguity caused by similar but non-identical atomic constraints notes
// the pair (clang's note_ambiguous_atomic_constraints; WP-109). Partial
// specializations get no such note.
template <class T> void f(T) requires (sizeof(T) > 1) {}
template <class T> void f(T) requires (sizeof(T) > 1) && true {}
void g() { f(1); }

template <class T> concept C1 = sizeof(T) > 1;
template <class T> concept C2 = sizeof(T) > 1 && true;
template <class T> requires C1<T> void h(T) {}
template <class T> requires C2<T> void h(T) {}
void k() { h(1); }
template <class T> void fa(T) requires (sizeof(T) > 1) && true {}
template <class T> void fa(T) requires (sizeof(T) > 1) {}
void g() { fa(1); }
template <class T> void fb(T) requires (sizeof(T) > 1) && (sizeof(T) < 9) {}
template <class T> void fb(T) requires (sizeof(T) < 9) && (sizeof(T) > 1) && true {}
void g2() { fb(1); }
template <class T> struct S;
template <class T> requires (sizeof(T) > 1) struct S<T> {};
template <class T> requires (sizeof(T) > 1) && true struct S<T> {};
S<int> s;
