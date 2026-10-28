//@ expect: ast
const char *plain = "tab\there \"q\" \\ \x7f";
const char8_t *utf8 = u8"é€";
const char16_t *utf16 = u"aé\U0001F600";
const char32_t *utf32 = U"\U0001F600z";
const char *joined = "ab" "cd";
const char arr[] = "xyz";
