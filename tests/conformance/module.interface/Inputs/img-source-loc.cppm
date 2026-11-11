export module imgsl;
export namespace sl {
unsigned line_here() { return __builtin_LINE(); }
const char *func_here() { return __builtin_FUNCTION(); }
template <class T> unsigned tline() { return __builtin_LINE(); }
unsigned caller_line(unsigned l = __builtin_LINE()) { return l; }
unsigned use_line() { return caller_line(); }
struct Member {
  unsigned l = __builtin_LINE();
};
unsigned member_line() { return Member{}.l; }
} // namespace sl
