// [temp.arg.general]/1, [temp.alias]/2 (CWG1430): a template-id with a pack
// expansion argument is checked up to the expansion; there are too many
// arguments when the parameters end before it, and an alias template or a
// concept takes no expansion for a parameter that is not a pack (clang's
// "pack expansion used as argument for non-pack parameter"). Not an error
// for a class or variable template, whose arity waits for the expansion.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:31 error "too many template arguments for class template 'P2'"
//@ diag: 25:35 note "template is declared here"
//@ diag: 29:53 error "too many template arguments for variable template 'vt'"
//@ diag: 23:33 note "template is declared here"
//@ diag: 32:34 error "pack expansion used as argument for non-pack parameter of alias template"
//@ diag: 30:16 note "template parameter is declared here"
//@ diag: 33:31 error "too many template arguments for alias template 'Al'"
//@ diag: 30:19 note "template is declared here"
//@ diag: 35:65 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:16 note "template parameter is declared here"
//@ diag: 36:35 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 24:16 note "template parameter is declared here"
//@ diag: 39:32 error "pack expansion used as argument for non-pack parameter of alias template"
//@ diag: 38:14 note "template parameter is declared here"
template<class T> constexpr int vt = 0;
template<class T> concept Co = true;
template<class T, class U> struct P2;
template<class... Ts> struct PV;
template<class... Ts> void p1(P2<Ts...>, P2<int, Ts...>, PV<Ts..., int>);
template<class... Ts> void p2(P2<int, int, Ts...>);
template<class... Ts> int p3() { return vt<Ts...> + vt<int, Ts...>; }
template<class T> using Al = T;
template<class T, class... Us> using AlV = T;
template<class... Ts> void p4(Al<Ts...>, AlV<int, Ts...>);
template<class... Ts> void p5(Al<int, Ts...>);
template<class T, class... Us> concept CoV = true;
template<class... Ts> bool p6() { return CoV<int, Ts...> && CoV<Ts...>; }
template<class... Ts> requires Co<Ts...> void p7();
template<int... Ns> struct NV;
template<int N> using AN = NV<N>;
template<int... Ns> void p8(AN<Ns...>);
