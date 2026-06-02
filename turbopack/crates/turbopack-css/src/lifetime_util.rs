use lightningcss::{
    stylesheet::{ParserOptions, StyleSheet},
    traits::IntoOwned,
};

pub fn stylesheet_into_static(
    ss: &StyleSheet<'_>,
    options: ParserOptions<'static>,
) -> StyleSheet<'static> {
    let sources = ss.sources.clone();
    let rules = ss.rules.clone().into_owned();

    StyleSheet::new(sources, rules, options)
}
