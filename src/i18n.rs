// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;

pub fn i18n_f(format: &str, args: &[(&str, &str)]) -> String {
    let s = gettext(format);
    freplace(s, args)
}

pub fn freplace(s: String, args: &[(&str, &str)]) -> String {
    let mut s = s;

    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_f() {
        let out = i18n_f("one and {arg}", &[("arg", "two")]);
        assert_eq!(out, "one and two");

        let out = i18n_f("{arg1}, {arg2}", &[("arg1", "one"), ("arg2", "two")]);
        assert_eq!(out, "one, two");
    }
}
