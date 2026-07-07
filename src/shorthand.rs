//! Class shorthand codebook.
//!
//! An *optional, opt-in* encoding for Tailwind class tokens: `ti4` decodes to
//! `text-indigo-400`, `fx` to `flex`. Two layers share one table so the two
//! directions can never drift:
//!
//! - a **generative grammar** for color utilities — `{property}{color}{shade}`
//!   concatenated with no separators (`bg-indigo-400` → `bi4`);
//! - a **spacing grammar** for the padding/margin/gap/size scale, dropping the
//!   hyphens (`px-4` → `px4`, `gap-x-6` → `gx6`, `-mt-4` → `-mt4`);
//! - a **curated table** for the most common non-color utilities (`rounded-full`
//!   → `rf`), matched as a whole token and taking precedence over the grammars.
//!
//! [`decode`] expands one token (compile path); [`encode`] contracts one class
//! (`html2fhtml`). [`encode`] emits a code *only if* it round-trips
//! (`decode(encode(c)) == Some(c)`), so contraction can never produce a token
//! that would decode to a different class — anything that doesn't round-trip is
//! left verbatim by the caller.
//!
//! **Variants** (`hover:`, `dark:`, `sm:`, stacked `dark:hover:…`) are handled
//! *transparently*: every `:`-separated variant segment is kept verbatim and
//! only the final base segment runs through the codebook (`hover:bg-blue-500`
//! → `hover:bb5`). Measurement (plan §2a) showed abbreviating the variant words
//! themselves saves nothing — `hover:` and a code like `hv:` both cost one BPE
//! token, and abbreviations fragment *worse* when stacked — so v1.1 leaves the
//! words alone and banks the base savings, at zero new ambiguity: the colon is
//! an unambiguous separator, so no new codebook is needed.

/// Curated non-color utilities: `(code, class)`. Codes and classes are both
/// unique (asserted in tests). Matched as an exact whole token, before the
/// grammar. Seeded from `bench/cheatsheet.md` / corpus frequency; grow on
/// demand — the grammar already covers the combinatorial color space.
const TABLE: &[(&str, &str)] = &[
    // display / layout
    ("fx", "flex"),
    ("ig", "inline-grid"),
    ("gr", "grid"),
    ("blk", "block"),
    ("ib", "inline-block"),
    ("hd", "hidden"),
    ("rel", "relative"),
    ("abs", "absolute"),
    // flex/grid alignment
    ("ic", "items-center"),
    ("is", "items-start"),
    ("ie", "items-end"),
    ("jc", "justify-center"),
    ("jb", "justify-between"),
    ("je", "justify-end"),
    ("js", "justify-start"),
    ("fc", "flex-col"),
    ("fwr", "flex-wrap"),
    ("f1", "flex-1"),
    // sizing
    ("wf", "w-full"),
    ("hf", "h-full"),
    ("ws", "w-screen"),
    ("hs", "h-screen"),
    ("mxa", "mx-auto"),
    // rounding
    ("rf", "rounded-full"),
    ("rd", "rounded"),
    ("rsm", "rounded-sm"),
    ("rmd", "rounded-md"),
    ("rl", "rounded-lg"),
    ("rx", "rounded-xl"),
    ("r2x", "rounded-2xl"),
    // shadow / border
    ("sh", "shadow"),
    ("shs", "shadow-sm"),
    ("shm", "shadow-md"),
    ("shl", "shadow-lg"),
    ("bo", "border"),
    // typography
    ("txs", "text-xs"),
    ("ts", "text-sm"),
    ("tb", "text-base"),
    ("tl", "text-lg"),
    ("txl", "text-xl"),
    ("t2x", "text-2xl"),
    ("fmd", "font-medium"),
    ("fsb", "font-semibold"),
    ("fb", "font-bold"),
    ("tc", "text-center"),
    ("tr", "text-right"),
    ("un", "underline"),
    ("tra", "truncate"),
    // position helpers
    ("ins", "inset-0"),
    ("po", "pointer-events-none"),
    ("cp", "cursor-pointer"),
    ("of", "overflow-hidden"),
    ("sr", "sr-only"),
];

/// Color-utility property prefixes: `(code, full)`. Order longest-first so the
/// greedy decode tries 2-char codes before 1-char; [`decode`] backtracks when a
/// longer prefix yields no valid parse (e.g. `to4` → `text-orange-400`, not the
/// dead end `to-` + `"4"`).
const PROPS: &[(&str, &str)] = &[
    ("bd", "border-"),
    ("dv", "divide-"),
    ("fm", "from-"),
    ("pl", "placeholder-"),
    ("vi", "via-"),
    ("to", "to-"),
    ("t", "text-"),
    ("b", "bg-"),
    ("r", "ring-"),
    ("o", "outline-"),
    ("f", "fill-"),
    ("s", "stroke-"),
];

/// Shaded color codes: `(code, name)`, longest-first for greedy prefix match.
const COLORS: &[(&str, &str)] = &[
    ("sl", "slate"),
    ("sk", "sky"),
    ("st", "stone"),
    ("gy", "gray"),
    ("gn", "green"),
    ("zn", "zinc"),
    ("ne", "neutral"),
    ("rd", "red"),
    ("ro", "rose"),
    ("am", "amber"),
    ("em", "emerald"),
    ("pu", "purple"),
    ("pk", "pink"),
    ("o", "orange"),
    ("y", "yellow"),
    ("l", "lime"),
    ("t", "teal"),
    ("c", "cyan"),
    ("b", "blue"),
    ("i", "indigo"),
    ("v", "violet"),
    ("f", "fuchsia"),
];

/// Shadeless colors: `black`/`white` carry no `-NNN` suffix (`text-white` → `tw`).
const COLORS_NOSHADE: &[(&str, &str)] = &[("bk", "black"), ("w", "white")];

/// Spacing/sizing property prefixes: `(code, full)`. The code drops the code
/// prefix's trailing hyphen; the value follows directly (`px-4` → `px4`,
/// `gap-x-6` → `gx6`). Order longest-first for greedy decode. `pl` overlaps the
/// color `placeholder-` prefix, but the value disambiguates: a numeric tail is
/// spacing, a color tail is the color grammar (which runs first).
const SPACING_PROPS: &[(&str, &str)] = &[
    ("spx", "space-x-"),
    ("spy", "space-y-"),
    ("px", "px-"),
    ("py", "py-"),
    ("pt", "pt-"),
    ("pr", "pr-"),
    ("pb", "pb-"),
    ("pl", "pl-"),
    ("mx", "mx-"),
    ("my", "my-"),
    ("mt", "mt-"),
    ("mr", "mr-"),
    ("mb", "mb-"),
    ("ml", "ml-"),
    ("gx", "gap-x-"),
    ("gy", "gap-y-"),
    ("sz", "size-"),
    ("p", "p-"),
    ("m", "m-"),
    ("g", "gap-"),
    ("w", "w-"),
    ("h", "h-"),
];

/// Tailwind's default spacing scale — the only values the spacing grammar
/// accepts, so encode/decode stay bijective. `px` is the literal 1px step;
/// fractions, `auto`, `full`, `screen` are left verbatim (or live in the table).
const SPACING_SCALE: &[&str] = &[
    "0", "0.5", "1", "1.5", "2", "2.5", "3", "3.5", "4", "5", "6", "7", "8", "9", "10", "11", "12",
    "14", "16", "20", "24", "28", "32", "36", "40", "44", "48", "52", "56", "60", "64", "72", "80",
    "96", "px",
];

/// Expands one shorthand token to its full class, or `None` if the token is not
/// recognized (the caller then leaves it verbatim). Table, then color grammar,
/// then spacing grammar.
pub fn decode(tok: &str) -> Option<String> {
    if tok.is_empty() {
        return None;
    }
    // Variant-transparent: keep the `:`-separated variant prefix verbatim,
    // decode only the base after the last colon (`hover:bb5` → `hover:...`).
    // Stacked variants fall out for free — `rsplit_once` peels one colon and
    // leaves the rest (`dark:hover`) in the prefix.
    if let Some((prefix, base)) = tok.rsplit_once(':') {
        return decode_base(base).map(|full| format!("{prefix}:{full}"));
    }
    decode_base(tok)
}

/// Decodes a base (variant-free) token: table, then color grammar, then spacing.
fn decode_base(tok: &str) -> Option<String> {
    if tok.is_empty() {
        return None;
    }
    if let Some(&(_, full)) = TABLE.iter().find(|(code, _)| *code == tok) {
        return Some(full.to_string());
    }
    decode_grammar(tok).or_else(|| decode_spacing(tok))
}

/// Decodes a spacing/sizing token, honoring a leading `-` for negative margins
/// and insets (`-mt4` → `-mt-4`).
fn decode_spacing(tok: &str) -> Option<String> {
    let (neg, body) = match tok.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", tok),
    };
    for (pcode, pfull) in SPACING_PROPS {
        let Some(val) = body.strip_prefix(pcode) else {
            continue;
        };
        if SPACING_SCALE.contains(&val) {
            return Some(format!("{neg}{pfull}{val}"));
        }
    }
    None
}

fn decode_grammar(tok: &str) -> Option<String> {
    for (pcode, pfull) in PROPS {
        let Some(rest) = tok.strip_prefix(pcode) else {
            continue;
        };
        // Shadeless colors must consume the whole remainder.
        if let Some(&(_, cname)) = COLORS_NOSHADE.iter().find(|(ccode, _)| *ccode == rest) {
            return Some(format!("{pfull}{cname}"));
        }
        // Shaded: a color prefix, then the rest is the entire shade.
        for (ccode, cname) in COLORS {
            let Some(shade_s) = rest.strip_prefix(ccode) else {
                continue;
            };
            if let Some(shade) = decode_shade(shade_s) {
                return Some(format!("{pfull}{cname}-{shade}"));
            }
        }
        // This property prefix led nowhere — backtrack to a shorter one.
    }
    None
}

/// Decodes a shade tail: a single digit `1..9` is ×100 (the common 100–900);
/// the rare `50`/`950` are written literally so no trailing zero is dropped.
fn decode_shade(s: &str) -> Option<u16> {
    match s {
        "50" => Some(50),
        "950" => Some(950),
        _ => match s.as_bytes() {
            [d @ b'1'..=b'9'] => Some((d - b'0') as u16 * 100),
            _ => None,
        },
    }
}

/// Contracts one full class to its shortest shorthand, but only when the result
/// round-trips. Returns `None` for anything not compressible (unknown class,
/// a variant with `:`, or a code that would decode to something else).
pub fn encode(class: &str) -> Option<String> {
    let cand = encode_raw(class)?;
    (decode(&cand).as_deref() == Some(class)).then_some(cand)
}

fn encode_raw(class: &str) -> Option<String> {
    // Variant-transparent: encode only the base behind the last colon, keep
    // the variant prefix verbatim (`hover:bg-blue-500` → `hover:bb5`).
    if let Some((prefix, base)) = class.rsplit_once(':') {
        return encode_base(base).map(|code| format!("{prefix}:{code}"));
    }
    encode_base(class)
}

fn encode_base(class: &str) -> Option<String> {
    if let Some(&(code, _)) = TABLE.iter().find(|(_, full)| *full == class) {
        return Some(code.to_string());
    }
    for (pcode, pfull) in PROPS {
        let Some(rest) = class.strip_prefix(pfull) else {
            continue;
        };
        if let Some(&(ccode, _)) = COLORS_NOSHADE.iter().find(|(_, name)| *name == rest) {
            return Some(format!("{pcode}{ccode}"));
        }
        if let Some((name, shade_s)) = rest.rsplit_once('-') {
            if let Some(&(ccode, _)) = COLORS.iter().find(|(_, n)| *n == name) {
                if let Some(scode) = encode_shade(shade_s) {
                    return Some(format!("{pcode}{ccode}{scode}"));
                }
            }
        }
    }
    encode_spacing(class)
}

fn encode_spacing(class: &str) -> Option<String> {
    let (neg, body) = match class.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", class),
    };
    for (pcode, pfull) in SPACING_PROPS {
        let Some(val) = body.strip_prefix(pfull) else {
            continue;
        };
        if SPACING_SCALE.contains(&val) {
            return Some(format!("{neg}{pcode}{val}"));
        }
    }
    None
}

fn encode_shade(s: &str) -> Option<String> {
    match s {
        "50" => Some("50".to_string()),
        "950" => Some("950".to_string()),
        _ => {
            let n: u16 = s.parse().ok()?;
            ((100..=900).contains(&n) && n.is_multiple_of(100)).then(|| (n / 100).to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const SHADES: &[u16] = &[50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

    /// Every point of the grammar domain round-trips both ways.
    #[test]
    fn grammar_round_trips_exhaustively() {
        for (_, pfull) in PROPS {
            for (_, cname) in COLORS {
                for shade in SHADES {
                    let class = format!("{pfull}{cname}-{shade}");
                    let code = encode(&class).unwrap_or_else(|| panic!("no encode for {class}"));
                    assert_eq!(
                        decode(&code).as_deref(),
                        Some(class.as_str()),
                        "code {code}"
                    );
                }
            }
            for (_, cname) in COLORS_NOSHADE {
                let class = format!("{pfull}{cname}");
                let code = encode(&class).unwrap_or_else(|| panic!("no encode for {class}"));
                assert_eq!(
                    decode(&code).as_deref(),
                    Some(class.as_str()),
                    "code {code}"
                );
            }
        }
    }

    /// Every table entry round-trips, and codes/classes are unique.
    #[test]
    fn table_round_trips_and_is_unique() {
        let mut codes = HashSet::new();
        let mut classes = HashSet::new();
        for (code, full) in TABLE {
            assert!(codes.insert(*code), "duplicate table code {code}");
            assert!(classes.insert(*full), "duplicate table class {full}");
            assert_eq!(decode(code).as_deref(), Some(*full));
            assert_eq!(encode(full).as_deref(), Some(*code), "class {full}");
        }
    }

    /// The awkward property/color letter-overlap cases decode as intended.
    #[test]
    fn property_color_overlap_backtracks() {
        assert_eq!(decode("to4").as_deref(), Some("text-orange-400"));
        assert_eq!(decode("toi4").as_deref(), Some("to-indigo-400"));
        assert_eq!(decode("bb5").as_deref(), Some("bg-blue-500"));
        assert_eq!(decode("bi4").as_deref(), Some("bg-indigo-400"));
        assert_eq!(decode("ti4").as_deref(), Some("text-indigo-400"));
        assert_eq!(decode("bdi4").as_deref(), Some("border-indigo-400"));
    }

    #[test]
    fn shade_edges() {
        assert_eq!(decode("ti50").as_deref(), Some("text-indigo-50"));
        assert_eq!(decode("ti950").as_deref(), Some("text-indigo-950"));
        assert_eq!(decode("ti5").as_deref(), Some("text-indigo-500"));
        assert_eq!(decode("ti9").as_deref(), Some("text-indigo-900"));
    }

    /// Every spacing prop × every scale value round-trips both ways.
    #[test]
    fn spacing_round_trips_exhaustively() {
        for (_, pfull) in SPACING_PROPS {
            for val in SPACING_SCALE {
                let class = format!("{pfull}{val}");
                let code = encode(&class).unwrap_or_else(|| panic!("no encode for {class}"));
                assert_eq!(
                    decode(&code).as_deref(),
                    Some(class.as_str()),
                    "code {code}"
                );
            }
        }
    }

    #[test]
    fn spacing_examples_and_negatives() {
        assert_eq!(decode("g4").as_deref(), Some("gap-4"));
        assert_eq!(decode("px4").as_deref(), Some("px-4"));
        assert_eq!(decode("gx6").as_deref(), Some("gap-x-6"));
        assert_eq!(decode("spy4").as_deref(), Some("space-y-4"));
        assert_eq!(decode("p0.5").as_deref(), Some("p-0.5"));
        assert_eq!(decode("mpx").as_deref(), Some("m-px"));
        // Negative margins keep the leading `-`.
        assert_eq!(decode("-mt4").as_deref(), Some("-mt-4"));
        assert_eq!(encode("-mt-4").as_deref(), Some("-mt4"));
        // Off-scale values are not spacing tokens.
        assert_eq!(decode("p13"), None);
        // `pl` + numeric is padding-left; `pl` + color is placeholder (color wins).
        assert_eq!(decode("pl2").as_deref(), Some("pl-2"));
        assert_eq!(decode("plsl4").as_deref(), Some("placeholder-slate-400"));
    }

    #[test]
    fn shadeless_colors() {
        assert_eq!(decode("tw").as_deref(), Some("text-white"));
        assert_eq!(decode("tbk").as_deref(), Some("text-black"));
        assert_eq!(encode("bg-white").as_deref(), Some("bw"));
    }

    #[test]
    fn non_shorthand_is_left_alone() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("w-1/2"), None);
        // Arbitrary values / unknown utilities stay verbatim.
        assert_eq!(encode("bg-[#0f172a]"), None);
        assert_eq!(encode("grid-cols-12"), None);
    }

    /// Variants keep their `:`-separated prefix verbatim and encode the base.
    #[test]
    fn variants_encode_base_only() {
        // Single variant, base via each grammar layer + the table.
        assert_eq!(encode("hover:bg-blue-500").as_deref(), Some("hover:bb5"));
        assert_eq!(decode("hover:bb5").as_deref(), Some("hover:bg-blue-500"));
        assert_eq!(encode("dark:text-white").as_deref(), Some("dark:tw"));
        assert_eq!(encode("sm:px-6").as_deref(), Some("sm:px6"));
        assert_eq!(
            encode("focus:ring-indigo-500").as_deref(),
            Some("focus:ri5")
        );
        assert_eq!(encode("hover:underline").as_deref(), Some("hover:un"));
        // Stacked variants: only the last colon is the base boundary.
        assert_eq!(
            encode("dark:hover:bg-slate-800").as_deref(),
            Some("dark:hover:bsl8")
        );
        assert_eq!(
            decode("dark:hover:bsl8").as_deref(),
            Some("dark:hover:bg-slate-800")
        );
        // Negative spacing behind a variant keeps its leading `-`.
        assert_eq!(encode("sm:-mt-4").as_deref(), Some("sm:-mt4"));
        assert_eq!(decode("sm:-mt4").as_deref(), Some("sm:-mt-4"));
        // Base that doesn't encode leaves the whole class verbatim.
        assert_eq!(encode("focus:outline-none"), None);
        assert_eq!(encode("sm:table-cell"), None);
        // Even an arbitrary `[…]` variant prefix is kept verbatim; only its
        // base is contracted, and it round-trips.
        assert_eq!(
            encode("data-[state=open]:bg-red-500").as_deref(),
            Some("data-[state=open]:brd5")
        );
        // A colon *inside* an arbitrary value (no real variant) must not
        // produce a bogus base — it simply stays verbatim.
        assert_eq!(encode("bg-[url(https://x)]"), None);
    }
}
