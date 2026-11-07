//! The network AI's strength as one Elo number: random play is 0 and each
//! value is the average over the 32 expansion modes for the shipped network.
//! Up to its greedy move (temperature 0) the AI samples the policy at a
//! temperature; above that it searches, with more simulations for more Elo.

/// The strongest setting: 4,096 simulations per move.
pub const MAX_ELO: u32 = 1410;
/// The policy's greedy move (temperature 0), without search.
pub const GREEDY_ELO: u32 = 860;

/// Policy play every 50 Elo from 0 to 850, as `β / (1 + β)` for the inverse
/// temperature β (0 is random); interpolating this keeps within 4 Elo.
const POLICY: [f64; 18] = [
    0.0, 0.0541, 0.1045, 0.1529, 0.1964, 0.2341, 0.2714, 0.3082, 0.3436, 0.3777, 0.4129, 0.4511,
    0.4902, 0.5321, 0.5793, 0.6340, 0.7020, 0.8132,
];
const POLICY_STEP: f64 = 50.0;
/// Elo of each measured search size; sizes in between are interpolated on
/// the logarithm. Four and eight simulations play alike.
const SEARCH: [(f64, f64); 11] = [
    (940.0, 4.0),
    (945.0, 8.0),
    (1030.0, 16.0),
    (1110.0, 32.0),
    (1150.0, 64.0),
    (1200.0, 128.0),
    (1250.0, 256.0),
    (1310.0, 512.0),
    (1350.0, 1024.0),
    (1390.0, 2048.0),
    (1410.0, 4096.0),
];
/// Settings between the greedy move and four simulations take the nearer.
const SEARCH_FROM: u32 = 900;

/// The simulations per move (0: no search) and policy temperature that play
/// at `elo` (capped at [`MAX_ELO`]).
pub fn setting(elo: u32) -> (usize, f64) {
    let elo = elo.min(MAX_ELO);
    let last = (POLICY.len() - 1) as f64 * POLICY_STEP;
    if f64::from(elo) <= last {
        let x = f64::from(elo) / POLICY_STEP;
        let i = (x as usize).min(POLICY.len() - 2);
        let u = POLICY[i] + (x - i as f64) * (POLICY[i + 1] - POLICY[i]);
        let temperature = if u > 0.0 {
            ((1.0 - u) / u * 1000.0).round() / 1000.0
        } else {
            f64::INFINITY
        };
        return (0, temperature);
    }
    if elo < SEARCH_FROM {
        return (0, 0.0);
    }
    let e = f64::from(elo);
    let (low, high) = SEARCH
        .windows(2)
        .map(|w| (w[0], w[1]))
        .find(|(_, high)| e <= high.0)
        .unwrap_or((SEARCH[SEARCH.len() - 2], SEARCH[SEARCH.len() - 1]));
    let t = ((e - low.0) / (high.0 - low.0)).clamp(0.0, 1.0);
    let sims = (low.1.log2() + t * (high.1.log2() - low.1.log2())).exp2();
    (sims.round() as usize, 0.0)
}

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
    fn elo_picks_a_temperature_below_the_greedy_move_and_search_above() {
        assert_eq!(setting(0), (0, f64::INFINITY));
        let (sims, temperature) = setting(400);
        assert_eq!(sims, 0);
        assert!((1.85..1.95).contains(&temperature), "{temperature}");
        assert_eq!(setting(GREEDY_ELO), (0, 0.0));
        assert_eq!(setting(899), (0, 0.0));
        assert_eq!(setting(900), (4, 0.0));
        assert_eq!(setting(1250), (256, 0.0));
        assert_eq!(setting(1200), (128, 0.0));
        assert_eq!(setting(MAX_ELO), (4096, 0.0));
        assert_eq!(setting(u32::MAX), (4096, 0.0));
        // Weaker settings never play stronger.
        let mut last = (0, f64::INFINITY);
        for elo in 0..=MAX_ELO {
            let now = setting(elo);
            assert!(
                now.0 > last.0 || (now.0 == last.0 && now.1 <= last.1),
                "{elo}: {now:?} after {last:?}"
            );
            last = now;
        }
    }

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
