// [dcl.attr.grammar]/6 (an implementation-defined attribute), [dcl.attr.unknown]:
// clang's thread-safety-analysis attributes (WP-606; spec/sema.md §4.5) are
// known and ignored in the GNU spelling `__attribute__((x(args)))` in every
// position clang accepts them -- class heads, data members (naming a member
// declared later), variables, functions before the declarator and after the
// parameter list, `const`, `noexcept` and a ref-qualifier, constructors,
// destructors, out-of-line definitions, templates and lambdas -- with their
// arguments (`this`, `!mu`, `*f()`, `mu.operator!()`) skipped unevaluated.
// No diagnostic, even under `-Werror -Wunknown-attributes`.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Werror -Wunknown-attributes
#define TS(x) __attribute__((x))

struct TS(capability("mutex")) Mutex {
  void Lock() TS(acquire_capability());
  void ReaderLock() TS(acquire_shared_capability());
  void Unlock() TS(release_capability());
  void ReaderUnlock() TS(release_shared_capability());
  void GenericUnlock() TS(release_generic_capability());
  bool TryLock() TS(try_acquire_capability(true));
  bool ReaderTryLock() TS(try_acquire_shared_capability(true));
  void AssertHeld() const TS(assert_capability(this));
  void AssertReaderHeld() const noexcept TS(assert_shared_capability(this));
  void OldLock() TS(exclusive_lock_function());
  void OldReaderLock() TS(shared_lock_function());
  bool OldTryLock() TS(exclusive_trylock_function(true));
  bool OldReaderTryLock() TS(shared_trylock_function(true));
  void OldUnlock() TS(unlock_function());
  void OldAssert() & TS(assert_exclusive_lock());
  void OldReaderAssert() && TS(assert_shared_lock());
  const Mutex &operator!() const;
};
struct TS(shared_capability("rwlock")) RW {};
class TS(lockable) OldMutex {};
// `reentrant_capability` is new in clang 22 (an unknown attribute in 19).
#if __clang_major__ >= 22
struct TS(reentrant_capability) TS(capability("recursive")) RecursiveMutex {};
#endif
class TS(scoped_lockable) Guard {
public:
  explicit Guard(Mutex *m) TS(acquire_capability(m)) : m_(m) {}
  ~Guard() TS(release_capability()) {}
private:
  Mutex *m_;
};

class Account {
  int balance_ TS(guarded_by(mu_));       // mu_ is declared below
  int *data_ TS(pt_guarded_by(mu_));
  int legacy_ TS(guarded_var);
  int *affixed_ __attribute__((__pt_guarded_by__(mu_), __pt_guarded_var__));
  int *plegacy_ TS(pt_guarded_var);
  Mutex mu_ TS(acquired_after(other_)) TS(acquired_before(third_));
  Mutex other_;
  Mutex third_;
public:
  void Deposit(int) TS(requires_capability(mu_));
  int Read() const TS(requires_shared_capability(mu_));
  void OldDeposit(int) TS(exclusive_locks_required(mu_, other_));
  int OldRead() const TS(shared_locks_required(mu_));
  void Outside() TS(locks_excluded(mu_));
  Mutex *GetMu() TS(lock_returned(mu_)) { return &mu_; }
  void Unsafe() TS(no_thread_safety_analysis) { balance_ = 0; }
  void Negated() TS(requires_capability(!mu_));
  void Call() TS(requires_capability(mu_.operator!()));
  void Deref() TS(requires_capability(*GetMu()));
  void Later() TS(requires_capability(later_mu_));
  Mutex later_mu_;
  TS(requires_capability(mu_)) void Before();
};

void Account::Deposit(int v) TS(no_thread_safety_analysis) { balance_ += v; }
TS(no_thread_safety_analysis) int Account::Read() const { return balance_; }

Mutex global_mu;
int global_x TS(guarded_by(global_mu));
TS(guarded_by(global_mu)) int global_y;
void LockGlobal() TS(acquire_capability(global_mu));
void UseGlobal() TS(requires_capability(global_mu)) {}

template <class T>
class TS(capability("templ")) TMutex {
  T value_ TS(guarded_by(this));
public:
  void Lock() TS(acquire_capability());
  template <class U> void Put(U u) TS(requires_capability(this)) { (void)u; }
};
template <class T> void TMutex<T>::Lock() TS(no_thread_safety_analysis) {}

template <class T>
void Locked(TMutex<T> &m) TS(requires_capability(m));

void f(Mutex &m TS(unused)) {
  Mutex local TS(unused);
  auto lam = [&]() TS(requires_capability(m)) {};
  lam();
}

int main() {
  TMutex<int> t;
  t.Lock();
  return 0;
}
