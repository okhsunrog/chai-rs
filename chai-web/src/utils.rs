/// Russian plural forms helper
///
/// Returns correct form based on count:
/// - 1, 21, 31, etc. -> one (but not 11)
/// - 2-4, 22-24, etc. -> few (but not 12-14)
/// - 0, 5-20, 25-30, etc. -> many
///
/// # Examples
/// ```
/// use chai_web::utils::russian_plural;
/// assert_eq!(russian_plural(1, "чай", "чая", "чаёв"), "чай");
/// assert_eq!(russian_plural(3, "чай", "чая", "чаёв"), "чая");
/// assert_eq!(russian_plural(5, "чай", "чая", "чаёв"), "чаёв");
/// assert_eq!(russian_plural(11, "чай", "чая", "чаёв"), "чаёв");
/// ```
#[must_use]
pub fn russian_plural<'a>(count: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let n = count % 100;
    if (11..=19).contains(&n) {
        many
    } else {
        match n % 10 {
            1 => one,
            2..=4 => few,
            _ => many,
        }
    }
}
