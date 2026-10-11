//! Language selection (English by default, French on a French system) and the
//! localized `--help` text.

use clap::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    /// `FREEPORT_LANG` (en|fr) wins, then the system locale, then English.
    pub fn detect() -> Lang {
        detect_from(
            std::env::var("FREEPORT_LANG").ok().as_deref(),
            sys_locale::get_locale().as_deref(),
        )
    }

    pub fn t<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }

    /// Same as [`Lang::t`] for already formatted strings.
    pub fn pick(self, en: String, fr: String) -> String {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }
}

pub fn detect_from(forced: Option<&str>, locale: Option<&str>) -> Lang {
    let pick = |s: &str| {
        if s.trim().to_ascii_lowercase().starts_with("fr") {
            Lang::Fr
        } else {
            Lang::En
        }
    };
    match forced.filter(|s| !s.trim().is_empty()) {
        Some(s) => pick(s),
        None => locale.map(pick).unwrap_or(Lang::En),
    }
}

/// The derive carries the English help; French replaces it at run time.
/// (clap's own parse errors stay in English.)
pub fn localize(cmd: Command, lang: Lang) -> Command {
    if lang == Lang::En {
        return cmd;
    }
    cmd.about("Libère un port en arrêtant le processus qui l'occupe")
        .help_template("{about-with-newline}\nUtilisation : {usage}\n\n{all-args}{after-help}")
        .mut_arg("ports", |a| {
            a.help("Port(s) ou plage(s) à libérer, ex. 3000 8080 3000-3010 (aucun : liste les ports en écoute)")
        })
        .mut_arg("list", |a| a.help("Liste les ports en écoute (par défaut sans argument)"))
        .mut_arg("dry_run", |a| a.help("Affiche les processus concernés sans rien arrêter"))
        .mut_arg("force", |a| {
            a.help("Arrêt immédiat et forcé (SIGKILL), sans arrêt propre préalable")
        })
        .mut_arg("udp", |a| a.help("Cible UDP au lieu de TCP"))
        .mut_arg("timeout", |a| {
            a.help("Délai (secondes) accordé à l'arrêt propre avant de forcer")
        })
        .mut_arg("interactive", |a| a.help("Demande confirmation avant chaque arrêt"))
        .mut_arg("addr", |a| a.help("Ne vise que les processus à l'écoute sur cette adresse IP"))
        .mut_arg("tree", |a| {
            a.help("Arrête aussi les superviseurs (qui relancent le service) et les processus enfants")
        })
        .mut_arg("allow_docker", |a| {
            a.help("Autorise l'arrêt d'un processus géré par Docker (déconseillé)")
        })
        .mut_arg("json", |a| a.help("Sortie JSON, pour les scripts"))
        .mut_arg("quiet", |a| a.help("N'affiche rien sur la sortie standard (erreurs sur stderr)"))
        .mut_arg("completions", |a| {
            a.help("Écrit le script de complétion du shell indiqué sur la sortie standard")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_language_wins() {
        assert_eq!(detect_from(Some("fr"), Some("en-US")), Lang::Fr);
        assert_eq!(detect_from(Some("EN"), Some("fr_FR.UTF-8")), Lang::En);
        assert_eq!(detect_from(Some("de"), Some("fr_FR")), Lang::En);
    }

    #[test]
    fn falls_back_to_locale_then_english() {
        assert_eq!(detect_from(None, Some("fr-FR")), Lang::Fr);
        assert_eq!(detect_from(None, Some("fr_CA.UTF-8")), Lang::Fr);
        assert_eq!(detect_from(None, Some("en-GB")), Lang::En);
        assert_eq!(detect_from(None, Some("C")), Lang::En);
        assert_eq!(detect_from(None, None), Lang::En);
        assert_eq!(detect_from(Some("  "), Some("fr-FR")), Lang::Fr);
    }
}
