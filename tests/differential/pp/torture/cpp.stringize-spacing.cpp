//@ expect: pp
// [cpp.stringize]: whitespace between the argument's tokens becomes one
// space, leading and trailing whitespace is dropped, and `"`/`\` inside
// string and character literals are escaped.
#define str(s) # s
#define xstr(s) str(s)
#define twice(x) x  +   x

str(a+b) str(a + b) str(a    +
       b) str(  leading) str(trailing   ) str(	 tabs	 )
str() str(   ) str(/* comment only */)
str("a\n" '\'' "\\" L"w" u8'c')
str('"') str("'")
xstr(twice(1)) str(twice(1))
xstr(__LINE__) str(__LINE__)
str(a/**/b) str(a/**/ /**/b) str(/**/a/**/)
str(@) str(` $x)
str(a
// comment between lines
b)
