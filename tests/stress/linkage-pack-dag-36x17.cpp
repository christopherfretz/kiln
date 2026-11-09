// [basic.link]/15, [temp.variadic], spec/abi.md §8.22: each `Foo<...>`
// level packs 36 copies of the previous one, so `Q` names 36^17 paths
// through 18 shared nodes. Emitting `Foo<P...>::Foo()` and its call to
// `ignore<P...>` asks for the linkage of every level's template
// arguments; it must be computed once per type, not once per path.
// Before WP-1076 four levels took over a minute. Reduced from clang's
// CodeGenCXX/pr29160.cpp, with `Base` defined so the program links.
//@ expect: run
//@ mem<=512
template <typename... Ts>
struct Foo {
  template <typename... T>
  static void ignore() {}
  Foo() { ignore<Ts...>(); }
};

struct Base {
  Base() {}
  ~Base() {}
};

#define STAMP(thiz, prev) using thiz = Foo< \
  prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, \
  prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, \
  prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, prev, prev \
  >;
STAMP(A, Base);
STAMP(B, A);
STAMP(C, B);
STAMP(D, C);
STAMP(E, D);
STAMP(F, E);
STAMP(G, F);
STAMP(H, G);
STAMP(I, H);
STAMP(J, I);
STAMP(K, J);
STAMP(L, K);
STAMP(M, L);
STAMP(N, M);
STAMP(O, N);
STAMP(P, O);
STAMP(Q, P);

int main() { Q q; }
