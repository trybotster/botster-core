//! The report order of client input (steward ruling on a5_2 s74/s75, R-88 group E; DESIGN.md "Report order"): the worker
//! writes a client frame's `ClientInput` report on the control link before any byte of that frame reaches the PTY. The link
//! is FIFO, so the PTY output that the frame causes reaches the host after the report.

use super::*;

/// Two client frames in one read: the second waits at the admission point until the first is written (AM-2). At every PTY
/// write, every frame whose bytes have reached the PTY so far has its report on the link already.
#[test]
fn a_client_frames_report_is_on_the_link_before_its_bytes_reach_the_pty() {
    let (mut w, _, _) = attached(limits());
    let mut stream = client_frame(&bytes_frame(1, b"ab"));
    stream.extend(client_frame(&bytes_frame(2, b"cd")));
    let mut all = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: stream,
    });
    all.extend(w.feed(Input::PtyWritten(Ok(2))));
    let mut reported = 0;
    let mut written = Vec::new();
    for action in &all {
        match action {
            Action::LinkSend(_) => {
                reported += w
                    .reports(std::slice::from_ref(action))
                    .iter()
                    .filter(|m| {
                        matches!(
                            m,
                            WorkerMsg::Observed {
                                observation: Observation::ClientInput {
                                    route: RouteId(1),
                                    ..
                                }
                            }
                        )
                    })
                    .count();
            }
            Action::PtyWrite(bytes) => {
                written.extend_from_slice(bytes);
                // "ab" is the first frame's, "cd" the second's.
                let frames_begun = if written.len() > 2 { 2 } else { 1 };
                assert!(
                    reported >= frames_begun,
                    "{frames_begun} frames reached the PTY with {reported} reports on the link"
                );
            }
            _ => {}
        }
    }
    assert_eq!(written, b"abcd");
    assert_eq!(reported, 2);
}
