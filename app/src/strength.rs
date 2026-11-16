//! The strength setting's text: reading an Elo from the setup field and
//! describing what a setting plays. The calibration itself is the engine's
//! [`inno_engine::strength`].

pub use inno_engine::strength::{GREEDY_ELO, MAX_ELO, setting};

/// Reads an Elo for the setup field; empty keeps a hand-set strength.
pub fn parse_elo(text: &str) -> Result<Option<u32>, String> {
    let text = text.trim().replace(',', "");
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<u32>() {
        Ok(elo) if elo <= MAX_ELO => Ok(Some(elo)),
        _ => Err(format!(
            "Enter a strength from 0 (random) to {MAX_ELO} Elo."
        )),
    }
}

/// What the network AI plays at `sims` and `temperature`.
pub fn describe(sims: usize, temperature: f64) -> String {
    if sims > 0 {
        format!("searches {sims} simulations per move")
    } else if temperature.is_infinite() {
        "samples policy network at temperature inf (random)".into()
    } else {
        format!("samples policy network at temperature {temperature}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elo_field_accepts_the_scale_or_nothing() {
        assert_eq!(parse_elo(" 1,250 "), Ok(Some(1250)));
        assert_eq!(parse_elo(""), Ok(None));
        assert!(parse_elo("1411").is_err() && parse_elo("-5").is_err() && parse_elo("x").is_err());
        assert_eq!(describe(256, 0.0), "searches 256 simulations per move");
        assert_eq!(describe(0, 0.0), "samples policy network at temperature 0");
        assert_eq!(
            describe(0, 1.5),
            "samples policy network at temperature 1.5"
        );
    }
}
