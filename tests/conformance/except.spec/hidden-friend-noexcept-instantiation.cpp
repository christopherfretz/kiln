// [temp.inst]/14, [except.spec]/13, [class.friend]/7: the dependent
// noexcept-specifier of a friend defined in a class template is
// instantiated only when needed, like a member's: it may name a private
// member declared after it (libc++ move_iterator's iter_move), and an
// error in it is reported with the point that needed it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:38 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:25 note "in instantiation of exception specification for 'bad' requested here"
template <class T> int g(T) noexcept;
template <class T> int h(T);
template <class I> struct It {
  friend int move(const It& i) noexcept(noexcept(g(i.cur))) { return 0; }
  friend int thr(const It& i) noexcept(noexcept(h(i.cur))) { return 0; }
  friend int get(const It& i) noexcept(noexcept(i.val())) { return 0; }
  friend int unused(const It&) noexcept(I::value) { return 0; }
  friend int bad(const It&) noexcept(I::value) { return 0; }
  int val() const noexcept(noexcept(g(cur))) { return 0; }
private:
  I cur;
};
It<int> it;
static_assert(noexcept(move(it)));
static_assert(!noexcept(thr(it)));
static_assert(noexcept(get(it)));
static_assert(!noexcept(bad(it)));
