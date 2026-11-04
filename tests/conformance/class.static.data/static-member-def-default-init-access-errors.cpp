// [class.static.data]/3, [class.access]/7: only the definition of the class's
// own static data member is in its scope; a namespace-scope variable, or a
// static data member of another class, default-initialized with a private
// constructor is an access error, for an explicit specialization as for a
// class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 error "calling a private constructor of class 'F<int>'"
//@ diag: 16:3 note "implicitly declared private here"
//@ diag: 25:3 error "calling a private constructor of class 'G'"
//@ diag: 20:26 note "implicitly declared private here"
//@ diag: 26:15 error "calling a private constructor of class 'F<int>'"
//@ diag: 16:3 note "implicitly declared private here"
template <typename T> class F;
template <> class F<int> {
  F() {}
  static F<int> inst;
};
F<int> F<int>::inst;
class G { static G inst; G() {} };
G G::inst;
struct Other { static F<int> f; };

F<int> outside;
G g;
F<int> Other::f;
