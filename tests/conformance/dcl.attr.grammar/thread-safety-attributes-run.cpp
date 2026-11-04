// [dcl.attr.grammar]/6: clang's thread-safety attributes (WP-606;
// spec/sema.md §4.5) attach nothing -- a lockable mutex, a scoped guard and
// guarded members annotated the way thread-annotation headers spell them
// (`#if defined(__clang__)`) compile under `-Werror` and run unchanged.
//@ expect: run
//@ strict-diags
//@ args: -Werror -Wunknown-attributes
//@ stdout: locked=1 balance=30 tries=1
#include <cstdio>

#if defined(__clang__)
#define THREAD_ANNOTATION(x) __attribute__((x))
#else
#define THREAD_ANNOTATION(x)
#endif
#define LOCKABLE THREAD_ANNOTATION(lockable)
#define SCOPED_LOCKABLE THREAD_ANNOTATION(scoped_lockable)
#define GUARDED_BY(x) THREAD_ANNOTATION(guarded_by(x))
#define EXCLUSIVE_LOCK_FUNCTION(...) THREAD_ANNOTATION(exclusive_lock_function(__VA_ARGS__))
#define EXCLUSIVE_TRYLOCK_FUNCTION(...) THREAD_ANNOTATION(exclusive_trylock_function(__VA_ARGS__))
#define UNLOCK_FUNCTION(...) THREAD_ANNOTATION(unlock_function(__VA_ARGS__))
#define EXCLUSIVE_LOCKS_REQUIRED(...) THREAD_ANNOTATION(exclusive_locks_required(__VA_ARGS__))
#define LOCKS_EXCLUDED(...) THREAD_ANNOTATION(locks_excluded(__VA_ARGS__))
#define ASSERT_EXCLUSIVE_LOCK(...) THREAD_ANNOTATION(assert_exclusive_lock(__VA_ARGS__))
#define NO_THREAD_SAFETY_ANALYSIS THREAD_ANNOTATION(no_thread_safety_analysis)

class LOCKABLE Mutex {
public:
  void Lock() EXCLUSIVE_LOCK_FUNCTION() { held_ = true; }
  bool TryLock() EXCLUSIVE_TRYLOCK_FUNCTION(true) {
    if (held_) return false;
    held_ = true;
    return true;
  }
  void Unlock() UNLOCK_FUNCTION() { held_ = false; }
  void AssertHeld() ASSERT_EXCLUSIVE_LOCK() {}
  bool held() const { return held_; }
private:
  bool held_ = false;
};

class SCOPED_LOCKABLE MutexLock {
public:
  explicit MutexLock(Mutex *mu) EXCLUSIVE_LOCK_FUNCTION(mu) : mu_(mu) { mu_->Lock(); }
  ~MutexLock() UNLOCK_FUNCTION() { mu_->Unlock(); }
private:
  Mutex *const mu_;
};

class Account {
public:
  void Deposit(int v) LOCKS_EXCLUDED(mu_) {
    MutexLock l(&mu_);
    AddLocked(v);
    locked_seen_ = mu_.held();
  }
  int Balance() NO_THREAD_SAFETY_ANALYSIS { return balance_; }
  bool locked_seen() const { return locked_seen_; }
  int Tries() LOCKS_EXCLUDED(mu_) {
    int n = 0;
    if (mu_.TryLock()) {
      ++n;
      if (mu_.TryLock()) ++n;  // already held: fails
      mu_.Unlock();
    }
    return n;
  }
private:
  void AddLocked(int v) EXCLUSIVE_LOCKS_REQUIRED(mu_) {
    mu_.AssertHeld();
    balance_ += v;
  }
  int balance_ GUARDED_BY(mu_) = 0;  // mu_ is declared below
  bool locked_seen_ = false;
  Mutex mu_;
};

int main() {
  Account a;
  a.Deposit(10);
  a.Deposit(20);
  std::printf("locked=%d balance=%d tries=%d\n", a.locked_seen() ? 1 : 0, a.Balance(), a.Tries());
  return 0;
}
