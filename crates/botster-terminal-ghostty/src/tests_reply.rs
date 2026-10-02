//! Tests of the typed replies. The library writes the bytes; the tests compare them with what the library itself
//! computes for the same state (the held shadow reply) and with the request that the program made.

use botster_core_contract::prelude::{CellPx, Size};
use botster_route_codec::prelude::{ColorScheme, QueryReply};

use super::reply::base64_decode;
use super::*;

fn terminal() -> Terminal {
    Terminal::new(&Size { rows: 24, cols: 80, cell_px: Some(CellPx { width: 9, height: 18 }) }, History::On).unwrap()
}

fn query(sequence: &[u8]) -> Query {
    terminal().vt_write_until_query(sequence).unwrap().query.unwrap()
}

#[test]
fn a_pixel_reply_is_what_the_library_computes_for_the_same_values() {
    // The held reply of the shadow comes from the library's own encoder, so equal values give equal bytes.
    for (sequence, width, height) in [(&b"\x1b[14t"[..], 80 * 9, 24 * 18), (b"\x1b[16t", 9, 18)] {
        let q = query(sequence);
        let bytes = q.reply_bytes(&QueryReply::Pixels { width, height }).unwrap();
        assert_eq!(bytes, q.shadow_reply, "{sequence:?}");
    }
}

#[test]
fn replies_carry_their_values() {
    let numbers = |bytes: &[u8]| -> Vec<u32> {
        bytes.split(|b| !b.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| std::str::from_utf8(s).unwrap().parse().unwrap()).collect()
    };
    let screen = query(b"\x1b[15t").reply_bytes(&QueryReply::Pixels { width: 1440, height: 900 }).unwrap();
    assert_eq!(&numbers(&screen)[1..], [900, 1440]);
    let chars = query(b"\x1b[19t").reply_bytes(&QueryReply::Chars { rows: 40, cols: 120 }).unwrap();
    assert_eq!(&numbers(&chars)[1..], [40, 120]);
    let position = query(b"\x1b[13t").reply_bytes(&QueryReply::Position { x: 12, y: 345 }).unwrap();
    assert_eq!(&numbers(&position)[1..], [12, 345]);
    // The state replies differ and are not empty.
    let shown = query(b"\x1b[11t").reply_bytes(&QueryReply::WindowState { iconified: false }).unwrap();
    let iconified = query(b"\x1b[11t").reply_bytes(&QueryReply::WindowState { iconified: true }).unwrap();
    assert!(!shown.is_empty() && !iconified.is_empty());
    assert_ne!(shown, iconified);
}

#[test]
fn a_position_above_the_wire_range_is_invalid() {
    let q = query(b"\x1b[13t");
    assert_eq!(q.reply_bytes(&QueryReply::Position { x: 65_536, y: 0 }), Err(ReplyError::Invalid));
    assert!(q.reply_bytes(&QueryReply::Position { x: 65_535, y: 65_535 }).is_ok());
}

#[test]
fn title_text_is_in_the_reply_and_a_control_character_is_invalid() {
    let title = query(b"\x1b[21t").reply_bytes(&QueryReply::Text { text: "my title".into() }).unwrap();
    assert!(title.windows(8).any(|w| w == b"my title"));
    let icon = query(b"\x1b[20t").reply_bytes(&QueryReply::Text { text: "my title".into() }).unwrap();
    assert_ne!(title, icon);
    assert_eq!(query(b"\x1b[21t").reply_bytes(&QueryReply::Text { text: "a\u{1b}b".into() }), Err(ReplyError::Invalid));
    assert_eq!(query(b"\x1b[21t").reply_bytes(&QueryReply::Text { text: "a\u{85}b".into() }), Err(ReplyError::Invalid));
}

#[test]
fn a_clipboard_reply_names_the_selection_and_ends_like_the_request() {
    for request in [&b"\x1b]52;p;?\x1b\\"[..], b"\x1b]52;p;?\x07"] {
        let q = query(request);
        let bytes = q.reply_bytes(&QueryReply::Clipboard { selection: "p".into(), data_base64: "Zm9vYmFy".into() }).unwrap();
        let tail = if request.ends_with(b"\x07") { 1 } else { 2 };
        assert!(bytes.ends_with(&request[request.len() - tail..]), "{request:?}");
        assert!(bytes.windows(8).any(|w| w == b"Zm9vYmFy"));
        assert!(bytes.windows(3).any(|w| w == b";p;"));
    }
    // An empty clipboard is a valid reply.
    let q = query(b"\x1b]52;c;?\x07");
    assert!(q.reply_bytes(&QueryReply::Clipboard { selection: "c".into(), data_base64: String::new() }).is_ok());
    // Another selection than the request's is not an answer to it.
    assert_eq!(q.reply_bytes(&QueryReply::Clipboard { selection: "p".into(), data_base64: String::new() }), Err(ReplyError::Mismatch));
    // Text that is not base64 is invalid.
    assert_eq!(q.reply_bytes(&QueryReply::Clipboard { selection: "c".into(), data_base64: "a*b=".into() }), Err(ReplyError::Invalid));
}

#[test]
fn the_color_scheme_reply_is_the_native_report_and_the_two_differ() {
    let q = query(b"\x1b[?996n");
    let dark = q.reply_bytes(&QueryReply::ColorScheme(ColorScheme::Dark)).unwrap();
    let light = q.reply_bytes(&QueryReply::ColorScheme(ColorScheme::Light)).unwrap();
    assert!(!dark.is_empty() && !light.is_empty());
    assert_ne!(dark, light);
}

#[test]
fn a_reply_must_answer_the_kind_of_its_query() {
    let cursor = query(b"\x1b[6n");
    for reply in [
        QueryReply::Pixels { width: 1, height: 1 },
        QueryReply::Chars { rows: 1, cols: 1 },
        QueryReply::WindowState { iconified: false },
        QueryReply::Position { x: 1, y: 1 },
        QueryReply::Text { text: "t".into() },
        QueryReply::ColorScheme(ColorScheme::Dark),
        QueryReply::Clipboard { selection: "c".into(), data_base64: String::new() },
    ] {
        assert_eq!(cursor.reply_bytes(&reply), Err(ReplyError::Mismatch), "{reply:?}");
    }
    // Pixels do not answer the character size.
    assert_eq!(query(b"\x1b[19t").reply_bytes(&QueryReply::Pixels { width: 1, height: 1 }), Err(ReplyError::Mismatch));
}

#[test]
fn a_decline_gives_no_bytes_and_encoded_bytes_pass_through() {
    let q = query(b"\x1b[6n");
    assert_eq!(q.reply_bytes(&QueryReply::Decline), Ok(Vec::new()));
    assert_eq!(q.reply_bytes(&QueryReply::Encoded { bytes_base64: "Zm9v".into() }), Ok(b"foo".to_vec()));
    assert_eq!(q.reply_bytes(&QueryReply::Encoded { bytes_base64: "Zm9".into() }), Err(ReplyError::Invalid));
}

#[test]
fn base64_decoding_follows_the_rfc_4648_vectors_and_rejects_the_rest() {
    for (text, bytes) in [("", &b""[..]), ("Zg==", b"f"), ("Zm8=", b"fo"), ("Zm9v", b"foo"), ("Zm9vYg==", b"foob"), ("Zm9vYmE=", b"fooba"), ("Zm9vYmFy", b"foobar")] {
        assert_eq!(base64_decode(text).unwrap(), bytes, "{text}");
    }
    assert_eq!(base64_decode("+/8=").unwrap(), [0xfb, 0xff]);
    for bad in ["Zg=", "Zg==Zg==", "Z===", "Zh==", "Zm9=", "Zm9v\n", "Zm 9", "Zm9-", "=Zm9"] {
        assert_eq!(base64_decode(bad), Err(ReplyError::Invalid), "{bad}");
    }
}
