use std::path::PathBuf;

use rawzelia_domain::CaptureDate;
use rawzelia_io::{IoError, PathTemplate};

#[test]
fn default_template_renders_session_path() {
    let rendered = PathTemplate::default()
        .render(
            CaptureDate {
                year: 2026,
                month: 10,
                day: 13,
                hour: 9,
                minute: 30,
                second: 0,
            },
            "Mariage Léa",
            "DSC_0001.NEF",
        )
        .expect("default path should render");

    assert_eq!(
        rendered,
        PathBuf::from("Photos/2026/2026-10-13 Mariage Léa/DSC_0001.NEF")
    );
}

#[test]
fn path_components_cannot_escape_the_rendered_destination() {
    let date = CaptureDate {
        year: 2026,
        month: 10,
        day: 13,
        hour: 0,
        minute: 0,
        second: 0,
    };

    assert!(matches!(
        PathTemplate::default().render(date, "../escape", "DSC_0001.NEF"),
        Err(IoError::Io(_))
    ));
    assert!(matches!(
        PathTemplate::default().render(date, "Session", "../../escape.NEF"),
        Err(IoError::Io(_))
    ));
}
