// [class.temporary]/3 with clang's `[[clang::trivial_abi]]` (WP-460):
// triviality for calls propagates to a class through its bases and
// members (K, M, T1<A>, a union); a dropped attribute (clang
// `checkIllFormedTrivialABIStruct`) still leaves an implicit member that
// was declared at `}` before the drop trivial for calls (F, G, H, T1<N>),
// so those classes pass in registers too, as with clang. The callee
// references and runs each by-value parameter's destructor.
struct N { N(const N &); long v; };
struct [[clang::trivial_abi]] A { A(const A &); ~A(); long v; };
struct [[clang::trivial_abi]] F : virtual A {};
struct [[clang::trivial_abi]] G { N n; };
struct [[clang::trivial_abi]] H : N {};
struct [[clang::trivial_abi]] J { N n; ~J(); };
struct [[clang::trivial_abi]] K { A a; N n; };
struct M { G g; };
union [[clang::trivial_abi]] UU { A a; long l; UU(const UU &); ~UU(); };
template <class T> struct [[clang::trivial_abi]] T1 { T t; };
void f(F) {}
void g(G) {}
void h(H) {}
void j(J) {}
void k(K) {}
void m(M) {}
void uu(UU) {}
void t1(T1<N>) {}
void t2(T1<A>) {}
void call_k(K &x) { k(x); }
