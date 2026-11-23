// Names in the arguments of GNU thread-safety attributes (spec/ast.md
// §18.8, WP-2170): kiln knows these attributes and does not analyse them
// (WP-606), but while the table records it parses their arguments for
// their names alone, as clang parses them: an attribute of a class member
// once the class is complete, so it names members declared after it
// (clang's late-parsed attributes; [dcl.attr.grammar]/6 leaves the
// arguments of an implementation-defined attribute to the implementation),
// a function's with its parameters in scope, anything else where it is
// written. `this` and `!e` are ordinary expressions. With the table off the
// arguments are skipped as before.
//@ written-refs: attr-thread-safety.refs
#define TS(x) __attribute__((x))
struct TS(capability("mutex")) Mutex {
  void Lock() TS(acquire_capability());
  void Unlock() TS(release_capability());
  bool TryLock() TS(try_acquire_capability(true));
  void AssertHeld() const TS(assert_capability(this));
  const Mutex &operator!() const;
};
Mutex global_mu;
Mutex other_mu TS(acquired_after(global_mu));
int counter TS(guarded_by(global_mu));
int *ptr TS(pt_guarded_by(global_mu));
void with_global() TS(requires_capability(global_mu));
void with_param(Mutex &m) TS(requires_capability(m));
void negative() TS(locks_excluded(!global_mu));
Mutex &get_mu() TS(lock_returned(global_mu));
struct Account {
  int balance TS(guarded_by(mu_));
  void Deposit(int n) TS(requires_capability(mu_)) { balance += n; }
  void Transfer(Account &to) TS(requires_capability(mu_, to.mu_));
  bool Try() TS(exclusive_trylock_function(true, mu_));
  void Both() TS(requires_shared_capability(this->mu_, Account::mu_));
  Mutex mu_;
  static Mutex shared_mu;
  void Static() TS(requires_capability(shared_mu));
  struct Inner {
    int x TS(guarded_by(inner_mu));
    Mutex inner_mu;
  };
};
template <class T> struct Guarded {
  T value TS(guarded_by(mu));
  Mutex mu;
};
Guarded<int> g;
