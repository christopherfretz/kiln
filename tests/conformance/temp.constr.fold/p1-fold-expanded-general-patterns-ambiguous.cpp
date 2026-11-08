// [temp.constr.fold]/4, [temp.constr.order]/1 (P2963R3): fold expanded
// constraints with general patterns subsume only when their patterns do:
// `(B<Box<Ts>> && ...)` does not subsume `(A<Ts> && ...)` (different
// atoms), an `&&` fold does not subsume an `||` fold, and `B<Box<Ts>>`
// does not subsume `A<Ts*>`. Each call is ambiguous
// (spec/sema-templates.md §8.2). WP-776.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 30:9 error "call to 'f' is ambiguous"
//@ diag: 23:62 note "candidate function [with Ts = <int, int>]"
//@ diag: 24:67 note "candidate function [with Ts = <int, int>]"
//@ diag: 31:9 error "call to 'g' is ambiguous"
//@ diag: 25:67 note "candidate function [with Ts = <int, int>]"
//@ diag: 26:67 note "candidate function [with Ts = <int, int>]"
//@ diag: 32:9 error "call to 'h' is ambiguous"
//@ diag: 27:63 note "candidate function [with Ts = <int, int>]"
//@ diag: 28:67 note "candidate function [with Ts = <int, int>]"
template <class T> struct Box { using type = T; };
template <class T> concept A = sizeof(T) > 0;
template <class T> concept B = A<T> && (alignof(T) > 0);

template <class... Ts> requires (A<Ts> && ...) constexpr int f(Ts...) { return 1; }
template <class... Ts> requires (B<Box<Ts>> && ...) constexpr int f(Ts...) { return 2; }
template <class... Ts> requires (A<Box<Ts>> && ...) constexpr int g(Ts...) { return 1; }
template <class... Ts> requires (B<Box<Ts>> || ...) constexpr int g(Ts...) { return 2; }
template <class... Ts> requires (A<Ts*> && ...) constexpr int h(Ts...) { return 1; }
template <class... Ts> requires (B<Box<Ts>> && ...) constexpr int h(Ts...) { return 2; }

int x = f(1, 2);
int y = g(1, 2);
int z = h(1, 2);
