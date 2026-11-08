//! Japanese-English dictionary: building the SQLite database (JMdict + Kanjium), deinflection, lookup at a position, pitch accent and furigana.
//!
//! * [`build`] turns the JMdict JSON (jmdict-simplified) and Kanjium's `accents.txt` into one SQLite file.
//! * [`Dict`] opens that file read-only; [`Dict::lookup_at`] is the click handler's workhorse.
//! * [`deinflect`] reduces conjugated forms to dictionary forms with a rule table written for this project.
//! * [`furigana()`] and [`pitch`] produce the ruby segments and pitch patterns shown in the popup.
//!
//! The serialized types ([`LookupResult`], [`DictEntry`], ...) match `src/lib/backend.ts` exactly.

pub mod build;
pub mod deinflect;
mod dict;
mod error;
pub mod furigana;
pub mod kana;
pub mod pitch;
mod store;
mod types;

pub use dict::Dict;
pub use error::Error;
pub use furigana::furigana;
pub use types::{DictEntry, Furigana, LookupResult, Pitch, Sense};
