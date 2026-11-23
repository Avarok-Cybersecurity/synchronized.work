#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The host's theme tokens to the App's colours: the forms the workspace host sends, the
//! fallbacks, and which token paints what.

use std::collections::BTreeMap;

use sw_app::config::{self, Token};
use sw_app::theme::{parse_color, theme_of, token};
use sw_graph::Motion;

#[test]
fn the_hosts_hsl_tokens_and_hex_colours_parse() {
    assert_eq!(parse_color("hsl(0 0% 100%)"), Some(0xFFFF_FFFF));
    assert_eq!(parse_color("hsl(257 45% 45%)"), Some(0x5C3F_A6FF));
    assert_eq!(parse_color("hsl(235 18% 13%)"), Some(0x1B1C_27FF));
    assert_eq!(parse_color("hsl(0, 72%, 45%)"), Some(0xC520_20FF));
    assert_eq!(parse_color("HSLA(0 0% 0% / 0.5)"), Some(0x0000_0080));
    assert_eq!(parse_color("hsla(120, 100%, 25%, 1)"), Some(0x0080_00FF));
    assert_eq!(parse_color("#76b900"), Some(0x76B9_00FF));
    assert_eq!(parse_color("#fff"), Some(0xFFFF_FFFF));
    assert_eq!(parse_color("#11223344"), Some(0x1122_3344));
}

#[test]
fn what_is_not_a_colour_is_refused() {
    for css in [
        "",
        "red",
        "hsl(257 45 45)",
        "hsl(1 2% 3% 4% 5%)",
        "#12345",
        "#ggg",
        "rgb(0 0 0)",
    ] {
        assert_eq!(parse_color(css), None, "{css}");
    }
}

#[test]
fn a_missing_or_unreadable_token_takes_its_fallback_and_a_sent_one_wins() {
    let none = BTreeMap::new();
    for t in Token::ALL {
        assert_eq!(token(&none, t), t.fallback().rgba(1.0), "{t:?}");
    }
    let bad = BTreeMap::from([("primary".to_owned(), "not a colour".to_owned())]);
    assert_eq!(token(&bad, Token::Primary), 0x5C3F_A6FF);
    let sent = BTreeMap::from([("primary".to_owned(), "hsl(0 0% 0%)".to_owned())]);
    assert_eq!(token(&sent, Token::Primary), 0x0000_00FF);
}

#[test]
fn the_fallbacks_are_the_workspace_uis_light_theme() {
    let want = [
        (Token::Background, 0xFFFF_FFFF),
        (Token::Foreground, 0x1B1C_27FF),
        (Token::Primary, 0x5C3F_A6FF),
        (Token::PrimaryForeground, 0xFFFF_FFFF),
        (Token::Muted, 0xF3F3_F7FF),
        (Token::MutedForeground, 0x5557_6DFF),
        (Token::Border, 0xDCDC_E5FF),
        (Token::Destructive, 0xC520_20FF),
    ];
    assert_eq!(want.len(), Token::ALL.len());
    for (t, rgba) in want {
        assert_eq!(t.fallback().rgba(1.0), rgba, "{t:?}");
    }
}

#[test]
fn each_token_paints_what_it_names() {
    let tokens: BTreeMap<String, String> = Token::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name().to_owned(), format!("#0000{:02x}", i + 1)))
        .collect();
    let c = |t: Token| token(&tokens, t);
    let theme = theme_of(&tokens, Motion::Lite);
    assert_eq!(theme.graph.background, c(Token::Background));
    assert_eq!(theme.graph.edge, c(Token::Border));
    assert_eq!(theme.graph.project, c(Token::Primary));
    assert_eq!(theme.graph.label, c(Token::Foreground));
    assert_eq!(theme.graph.agent, config::BRAND_GREEN);
    assert_eq!(theme.text, c(Token::Foreground));
    assert_eq!(theme.muted, c(Token::MutedForeground));
    assert_eq!(theme.error, c(Token::Destructive));
    assert_eq!(
        (theme.button, theme.button_text),
        (c(Token::Primary), c(Token::PrimaryForeground))
    );
    assert_eq!(theme.field, c(Token::Muted));
    assert_eq!(theme.motion, Motion::Lite);
    assert!(
        theme.button_size.1 >= 44.0,
        "tap targets are at least 44 px"
    );
}
