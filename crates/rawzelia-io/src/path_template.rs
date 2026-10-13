use std::path::{Component, Path, PathBuf};

use rawzelia_domain::CaptureDate;

use crate::IoError;

const DEFAULT_TEMPLATE: &str = "Photos/{year}/{date} {session}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathTemplate(String);

impl PathTemplate {
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self(DEFAULT_TEMPLATE.to_owned())
    }

    pub fn render(
        &self,
        date: CaptureDate,
        session_name: &str,
        original_name: &str,
    ) -> Result<PathBuf, IoError> {
        if self.0 != DEFAULT_TEMPLATE {
            return Err(IoError::Io("unsupported path template".to_owned()));
        }
        validate_date(date)?;
        require_safe_component(session_name, "session name")?;
        require_safe_component(original_name, "original name")?;

        let year = format!("{:04}", date.year);
        let dated_session = format!(
            "{:04}-{:02}-{:02} {session_name}",
            date.year, date.month, date.day
        );
        Ok(PathBuf::from("Photos")
            .join(year)
            .join(dated_session)
            .join(original_name))
    }
}

impl Default for PathTemplate {
    fn default() -> Self {
        Self(DEFAULT_TEMPLATE.to_owned())
    }
}

fn require_safe_component(value: &str, label: &str) -> Result<(), IoError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains(['/', '\\'])
        || value.chars().any(char::is_control)
    {
        return Err(IoError::Io(format!("invalid {label}")));
    }

    let mut components = Path::new(value).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(IoError::Io(format!("invalid {label}")));
    }
    Ok(())
}

fn validate_date(date: CaptureDate) -> Result<(), IoError> {
    let leap = date.year % 4 == 0 && (date.year % 100 != 0 || date.year % 400 == 0);
    let maximum_day = match date.month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if date.year < 1
        || date.day == 0
        || date.day > maximum_day
        || date.hour > 23
        || date.minute > 59
        || date.second > 59
    {
        return Err(IoError::Io("invalid capture date".to_owned()));
    }
    Ok(())
}
