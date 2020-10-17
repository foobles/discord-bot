use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};

#[derive(Serialize, Deserialize)]
pub struct StrCow<'a>(#[serde(borrow)] Cow<'a, str>);

impl<'a> Debug for StrCow<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} ({})",
            self.as_str(),
            if let Cow::Borrowed(_) = self.get_ref() {
                "borrowed"
            } else {
                "owned"
            }
        )
    }
}

impl<'a> AsRef<str> for StrCow<'a> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'a> StrCow<'a> {
    pub fn into_cow(self) -> Cow<'a, str> {
        self.0
    }

    pub fn get_ref(&self) -> &Cow<'a, str> {
        &self.0
    }

    pub fn get_mut(&mut self) -> &mut Cow<'a, str> {
        &mut self.0
    }

    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    pub fn from_str(s: &'a str) -> Self {
        StrCow(Cow::Borrowed(s))
    }

    pub fn from_string(s: String) -> Self {
        StrCow(Cow::Owned(s))
    }

    pub fn from_cow(cow: Cow<'a, str>) -> Self {
        StrCow(cow)
    }
}
