// String literals: narrow, wide, UTF-8/16/32, escapes and embedded nulls.
int use(const void*);
int strings() {
  const char* a = "plain\n\t\"quoted\"";
  const wchar_t* w = L"wide";
  const char8_t* u8 = u8"utf8 é";
  const char16_t* u = u"utf16";
  const char32_t* U = U"utf32 \U0001F600";
  const char* nul = "a\0b";
  return use(a) + use(w) + use(u8) + use(u) + use(U) + use(nul) + sizeof("abc");
}
