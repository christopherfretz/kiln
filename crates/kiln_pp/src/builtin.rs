//! Builtin macros ([cpp.predefined] plus clang's extensions) and the
//! feature-query hook ([`FeatureQueries`]).
//!
//! Builtins live in the macro table (so `defined(__LINE__)` and
//! `#ifdef __has_include` are true, as in clang) with
//! [`crate::MacroDef::is_builtin`] set. The core computes the values
//! that depend only on the preprocessor (`__LINE__`, `__FILE__`,
//! `__COUNTER__`, `__has_include`, ...); everything that depends on the
//! language implementation or target (`__has_feature`,
//! `__has_builtin`, `__has_cpp_attribute`, `__is_target_os`, ...) is
//! asked of a [`FeatureQueries`] implementation (WP-017).

/// A builtin macro.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum Builtin {
    Line,
    File,
    Date,
    Time,
    Counter,
    IncludeLevel,
    BaseFile,
    FileName,
    Timestamp,
    FltEvalMethod,
    /// `_Pragma` ([cpp.pragma.op]).
    Pragma,
    HasInclude,
    HasIncludeNext,
    HasEmbed,
    HasFeature,
    HasExtension,
    HasBuiltin,
    HasConstexprBuiltin,
    HasAttribute,
    HasCppAttribute,
    HasDeclspecAttribute,
    HasWarning,
    IsIdentifier,
    IsTargetArch,
    IsTargetVendor,
    IsTargetOs,
    IsTargetEnvironment,
    IsTargetVariantOs,
    IsTargetVariantEnvironment,
}

impl Builtin {
    /// Every builtin with its name, in clang's registration order.
    pub const ALL: &'static [(&'static str, Builtin)] = &[
        ("__LINE__", Builtin::Line),
        ("__FILE__", Builtin::File),
        ("__DATE__", Builtin::Date),
        ("__TIME__", Builtin::Time),
        ("__COUNTER__", Builtin::Counter),
        ("_Pragma", Builtin::Pragma),
        ("__FLT_EVAL_METHOD__", Builtin::FltEvalMethod),
        ("__has_cpp_attribute", Builtin::HasCppAttribute),
        ("__BASE_FILE__", Builtin::BaseFile),
        ("__INCLUDE_LEVEL__", Builtin::IncludeLevel),
        ("__TIMESTAMP__", Builtin::Timestamp),
        ("__FILE_NAME__", Builtin::FileName),
        ("__has_feature", Builtin::HasFeature),
        ("__has_extension", Builtin::HasExtension),
        ("__has_builtin", Builtin::HasBuiltin),
        ("__has_constexpr_builtin", Builtin::HasConstexprBuiltin),
        ("__has_attribute", Builtin::HasAttribute),
        ("__has_declspec_attribute", Builtin::HasDeclspecAttribute),
        ("__has_embed", Builtin::HasEmbed),
        ("__has_include", Builtin::HasInclude),
        ("__has_include_next", Builtin::HasIncludeNext),
        ("__has_warning", Builtin::HasWarning),
        ("__is_identifier", Builtin::IsIdentifier),
        ("__is_target_arch", Builtin::IsTargetArch),
        ("__is_target_vendor", Builtin::IsTargetVendor),
        ("__is_target_os", Builtin::IsTargetOs),
        ("__is_target_environment", Builtin::IsTargetEnvironment),
        ("__is_target_variant_os", Builtin::IsTargetVariantOs),
        ("__is_target_variant_environment", Builtin::IsTargetVariantEnvironment),
    ];

    /// The macro name.
    pub fn name(self) -> &'static str {
        Builtin::ALL.iter().find(|&&(_, b)| b == self).map_or("", |&(n, _)| n)
    }
}

/// Which `__is_target_*` query.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TargetQuery {
    /// `__is_target_arch`.
    Arch,
    /// `__is_target_vendor`.
    Vendor,
    /// `__is_target_os`.
    Os,
    /// `__is_target_environment`.
    Environment,
    /// `__is_target_variant_os`.
    VariantOs,
    /// `__is_target_variant_environment`.
    VariantEnvironment,
}

/// Answers the language- and target-dependent builtin queries.
///
/// To be implemented by WP-017 (feature tables generated from clang 19
/// for the configured target). The defaults are those of a compiler that
/// supports nothing beyond the standard: no features, extensions or
/// builtins; the standard attributes of [cpp.cond] Table 23 (with
/// clang 19's values); the C++26 keywords.
pub trait FeatureQueries {
    /// `__has_feature(name)`.
    fn has_feature(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// `__has_extension(name)` (clang: features are also extensions).
    fn has_extension(&self, name: &str) -> bool {
        self.has_feature(name)
    }

    /// `__has_builtin(name)`: 0 if absent, else the value to produce
    /// (clang gives 201802 for `__builtin_operator_new`, 1 otherwise).
    /// The core answers `__is_target_*` itself.
    fn has_builtin(&self, name: &str) -> u32 {
        let _ = name;
        0
    }

    /// `__has_constexpr_builtin(name)`.
    fn has_constexpr_builtin(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// `__has_attribute(name)` (GNU `__attribute__` syntax).
    fn has_attribute(&self, name: &str) -> u32 {
        let _ = name;
        0
    }

    /// `__has_cpp_attribute(scope::name)` / `__has_cpp_attribute(name)`.
    fn has_cpp_attribute(&self, scope: Option<&str>, name: &str) -> u32 {
        if scope.is_some() {
            return 0;
        }
        let name = name.strip_prefix("__").and_then(|n| n.strip_suffix("__")).unwrap_or(name);
        match name {
            "assume" => 202_207,
            "carries_dependency" | "noreturn" => 200_809,
            "deprecated" => 201_309,
            "fallthrough" | "maybe_unused" => 201_603,
            "likely" | "unlikely" | "no_unique_address" => 201_803,
            "nodiscard" => 201_907,
            _ => 0,
        }
    }

    /// `__has_declspec_attribute(name)`.
    fn has_declspec_attribute(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// True if `name` is a keyword (or alternative token) of the language:
    /// `__is_identifier(name)` is `!is_keyword(name)`.
    fn is_keyword(&self, name: &str) -> bool {
        CXX26_KEYWORDS.binary_search(&name).is_ok()
    }

    /// `__is_target_arch(name)` and friends.
    fn is_target(&self, query: TargetQuery, name: &str) -> bool {
        let _ = (query, name);
        false
    }

    /// The value of `__FLT_EVAL_METHOD__`.
    fn flt_eval_method(&self) -> i32 {
        0
    }
}

/// The default [`FeatureQueries`]: the trait defaults.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFeatures;

impl FeatureQueries for NoFeatures {}

/// C++26 keywords ([lex.key] Table 5) and alternative tokens
/// ([lex.digraph] Table 6), sorted.
const CXX26_KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char16_t",
    "char32_t",
    "char8_t",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "compl",
    "concept",
    "const",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "continue",
    "contract_assert",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// The C++ alternative tokens that cannot be macro names
/// ([lex.digraph], [cpp.replace.general]/9), with the token each stands for.
pub(crate) const OPERATOR_NAMES: &[(&str, &str)] = &[
    ("and", "&&"),
    ("and_eq", "&="),
    ("bitand", "&"),
    ("bitor", "|"),
    ("compl", "~"),
    ("not", "!"),
    ("not_eq", "!="),
    ("or", "||"),
    ("or_eq", "|="),
    ("xor", "^"),
    ("xor_eq", "^="),
];

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `__DATE__` and `__TIME__` spellings (without quotes) for `secs`
/// seconds since the epoch (UTC; clang uses local time, and the
/// container runs UTC).
pub(crate) fn date_time(secs: i64) -> (String, String) {
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let date = format!("{} {d:2} {y:4}", MONTHS[(m - 1) as usize]);
    let time = format!("{:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60);
    (date, time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_sorted() {
        assert!(CXX26_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn dates() {
        assert_eq!(date_time(0), ("Jan  1 1970".to_string(), "00:00:00".to_string()));
        assert_eq!(date_time(1_700_000_000), ("Nov 14 2023".to_string(), "22:13:20".to_string()));
    }
}
