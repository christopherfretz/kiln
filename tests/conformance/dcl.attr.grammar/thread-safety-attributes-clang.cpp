// [dcl.attr.grammar]/4, /6 (an attribute-namespace names an
// implementation-defined attribute): the thread-safety attributes clang
// spells `Clang<"x">` (WP-606; spec/sema.md §4.5) are known and ignored as
// `[[clang::x(args)]]` (and `[[_Clang::x]]`, `[[clang::__x__]]`) on classes,
// data members, variables and functions, including out-of-line definitions
// and templates. No diagnostic, even under `-Werror -Wunknown-attributes`.
// Unlike a GNU attribute in a class, clang parses these arguments where they
// are written, so they name only what is declared before them. The GNU-only
// names are unknown in this spelling (thread-safety-attributes-unknown.cpp).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Werror -Wunknown-attributes

struct [[clang::capability("mutex")]] Mutex {
  [[clang::acquire_capability()]] void Lock();
  [[clang::acquire_shared_capability()]] void ReaderLock();
  [[clang::release_capability()]] void Unlock();
  [[clang::release_shared_capability()]] void ReaderUnlock();
  [[clang::release_generic_capability()]] void GenericUnlock();
  [[clang::unlock_function()]] void OldUnlock();
  [[clang::try_acquire_capability(true)]] bool TryLock();
  [[clang::try_acquire_shared_capability(true)]] bool ReaderTryLock();
  [[clang::assert_capability()]] void AssertHeld() const;
  [[clang::assert_shared_capability()]] void AssertReaderHeld() const;
  const Mutex &operator!() const;
};
struct [[clang::shared_capability("rwlock")]] RW {};
struct [[_Clang::__capability__("reserved")]] Reserved {};
Mutex global_mu;
class [[clang::scoped_lockable]] Guard {
public:
  [[clang::acquire_capability(global_mu)]] explicit Guard(Mutex *m) : m_(m) {}
  [[clang::release_capability()]] ~Guard() {}
private:
  Mutex *m_;
};
#if __clang_major__ >= 22
struct [[clang::reentrant_capability, clang::capability("recursive")]] Recursive {};
#endif

class Account {
  Mutex mu_;
  Mutex other_;
  [[clang::guarded_var]] int legacy_;
  [[clang::pt_guarded_var]] int *plegacy_;
public:
  [[clang::requires_capability(mu_)]] void Deposit(int);
  [[clang::requires_shared_capability(mu_)]] int Read() const;
  [[clang::exclusive_locks_required(mu_, other_)]] void OldDeposit(int);
  [[clang::shared_locks_required(mu_)]] int OldRead() const;
  [[clang::no_thread_safety_analysis]] void Unsafe() { legacy_ = 0; }
  [[clang::requires_capability(!mu_)]] void Negated();
};

[[clang::no_thread_safety_analysis]] void Account::Deposit(int v) { legacy_ += v; }

[[clang::guarded_var]] int global_x;
[[clang::requires_capability(global_mu)]] void UseGlobal() {}

template <class T>
class [[clang::capability("templ")]] TMutex {
  [[clang::guarded_var]] T value_;
public:
  [[clang::acquire_capability()]] void Lock();
  template <class U> [[clang::requires_capability(value_)]] void Put(U) {}
};
template <class T> [[clang::no_thread_safety_analysis]] void TMutex<T>::Lock() {}

template <class T>
[[clang::requires_capability(global_mu)]] void Locked(TMutex<T> &m);

int main() {
  TMutex<int> t;
  t.Lock();
  return 0;
}
