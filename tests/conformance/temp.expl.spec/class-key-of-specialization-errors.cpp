// [class.access.base]/2, [class.access]/3, [temp.expl.spec],
// [temp.spec.partial.general]: the reverse of
// class-key-of-specialization-run.cpp. An explicit or partial
// specialization defined with `class` over a primary declared `struct`
// inherits privately and has private leading members, whatever the
// primary's key (WP-611).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 49:20 error "'V' is a private member of 'G<float>'"
//@ diag: 22:31 note "constrained by implicitly private inheritance here"
//@ diag: 21:38 note "member is declared here"
//@ diag: 50:19 error "'V' is a private member of 'G<int>'"
//@ diag: 23:35 note "constrained by implicitly private inheritance here"
//@ diag: 21:38 note "member is declared here"
//@ diag: 51:30 error "'a' is a private member of 'Y<int>'"
//@ diag: 31:7 note "implicitly declared private here"
//@ diag: 52:33 error "'a' is a private member of 'Y<char *>'"
//@ diag: 35:7 note "implicitly declared private here"
//@ diag: 53:30 error "'a' is a private member of 'Z<int>'"
//@ diag: 46:7 note "implicitly declared private here"
template <class L> struct G { enum { V = 4 }; };
template <> class G<double> : G<float> {};
template <class L> class G<L *> : G<L> {};

template <class T>
struct Y {
  int a;
};
template <>
class Y<int> {
  int a;
};
template <class T>
class Y<T *> {
  int a;
};

template <class T>
struct Z {
  int a;
};
template <>
struct Z<int>;
template <>
class Z<int> {
  int a;
};

int a = G<double>::V;
int b = G<int *>::V;
int f() { Y<int> y; return y.a; }
int g() { Y<char *> y; return y.a; }
int h() { Z<int> z; return z.a; }
