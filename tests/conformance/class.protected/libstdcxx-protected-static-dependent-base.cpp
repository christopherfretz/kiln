// [class.protected]/1, [class.access.base]/5: a protected static member is
// accessible from a member of a derived class through any qualified name,
// even when the derivation goes through a dependent base. libstdc++
// bits/messages_members.h:107 (messages_byname<_CharT> uses
// locale::facet::_S_get_c_name()); <locale> and 25 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): protected static member through a dependent base rejected
struct locale {
  class facet {
  protected:
    static const char *c_name() throw() { return "C"; }
  };
};
template<class T> struct messages : locale::facet {
  const char *name = "C";
};
template<class T> struct messages_byname : messages<T> {
  messages_byname(const char *s);
};
template<class T> messages_byname<T>::messages_byname(const char *s) {
  if (this->name != locale::facet::c_name()) this->name = s;
}
template struct messages_byname<char>;
