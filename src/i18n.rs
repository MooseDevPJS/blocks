//! Lingua dell'interfaccia: italiano o inglese.
//!
//! Ogni testo è scritto nel punto in cui si usa come coppia `tr("italiano", "english")`,
//! così le due versioni restano sempre vicine e non possono andare fuori sincrono.

use crate::settings::Language;
use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set(lang: Language) {
    let en = match lang {
        Language::It => false,
        Language::En => true,
        Language::Auto => !sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("it")),
    };
    ENGLISH.store(en, Ordering::Relaxed);
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn tr(it: &'static str, en: &'static str) -> &'static str {
    if is_english() { en } else { it }
}
