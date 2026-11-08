// [temp.constr.normal]/1.4, /1.5, [temp.constr.fold]/4 (P2963R3): a
// concept-id's normal form is its concept's with the concept-id's
// arguments substituted into the parameter mapping, so through a concept
// the fold expanded constraint is the one over the mapped pattern:
// `AllB<Box<Ts>...>` is `(B<Box<Ts>> && ...)` and subsumes the written
// `(A<Box<Ts>> && ...)` (`f`, `g`, `h` take the more constrained
// overload), while `AllA<Box<Ts>...>` and `AllB<Ts...>` have unrelated
// patterns (`A<Box<Ts>>`, `B<Ts>`) and neither subsumes (`i`..`l` are
// ambiguous). Kiln follows the wording (spec/sema-templates.md §8.2).
//@ skip(clang): clang 22 identifies a concept's fold elements by the concept's pack parameter, not the mapped pattern
//@ expect: compile-fail
//@ strict-diags
//@ diag: 52:10 error "call to 'i' is ambiguous"
//@ diag: 44:64 note "candidate function [with Ts = <int, int>]"
//@ diag: 45:59 note "candidate function [with Ts = <int, int>]"
//@ diag: 53:10 error "call to 'j' is ambiguous"
//@ diag: 46:59 note "candidate function [with Ts = <int, int>]"
//@ diag: 47:64 note "candidate function [with Ts = <int, int>]"
//@ diag: 54:10 error "call to 'k' is ambiguous"
//@ diag: 48:64 note "candidate function [with Ts = <int, int>]"
//@ diag: 49:60 note "candidate function [with Ts = <int, int>]"
//@ diag: 55:10 error "call to 'l' is ambiguous"
//@ diag: 50:64 note "candidate function [with Ts = <int, int>]"
//@ diag: 51:65 note "candidate function [with Ts = <int, int>]"
template <class T> struct Box { using type = T; };
template <class T> concept A = sizeof(T) > 0;
template <class T> concept B = A<T> && (alignof(T) > 0);
template <class T, class U> concept A2 = sizeof(T) >= sizeof(U) || true;
template <class T, class U> concept B2 = A2<T, U> && (alignof(T) > 0);
template <class... Us> concept AllA = (A<Us> && ...);
template <class... Us> concept AllB = (B<Us> && ...);
template <class... Us> concept AllA2 = (A2<Us, int> && ...);

template <class... Ts> requires (A<Box<Ts>> && ...) constexpr int f(Ts...) { return 1; }
template <class... Ts> requires AllB<Box<Ts>...> constexpr int f(Ts...) { return 2; }
static_assert(f(1, 2) == 2);
template <class... Ts> requires AllA<Ts*...> constexpr int g(Ts...) { return 1; }
template <class... Ts> requires (B<Ts*> && ...) constexpr int g(Ts...) { return 2; }
static_assert(g(1, 2) == 2);
template <class... Ts> requires AllA2<Box<Ts>...> constexpr int h(Ts...) { return 1; }
template <class... Ts> requires (B2<Box<Ts>, int> && ...) constexpr int h(Ts...) { return 2; }
static_assert(h(1, 2) == 2);

template <class... Ts> requires AllA<Box<Ts>...> constexpr int i(Ts...) { return 1; }
template <class... Ts> requires AllB<Ts...> constexpr int i(Ts...) { return 2; }
template <class... Ts> requires AllA<Ts...> constexpr int j(Ts...) { return 1; }
template <class... Ts> requires AllB<Box<Ts>...> constexpr int j(Ts...) { return 2; }
template <class... Ts> requires AllA<Box<Ts>...> constexpr int k(Ts...) { return 1; }
template <class... Ts> requires AllB<Ts*...> constexpr int k(Ts...) { return 2; }
template <class... Ts> requires AllA<Box<Ts>...> constexpr int l(Ts...) { return 1; }
template <class... Ts> requires AllB<Box<Ts>*...> constexpr int l(Ts...) { return 2; }
int wi = i(1, 2);
int wj = j(1, 2);
int wk = k(1, 2);
int wl = l(1, 2);
