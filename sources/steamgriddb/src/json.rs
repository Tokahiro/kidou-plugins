//! SteamGridDB's answers as JSON: `{success, data}` or `{success: false, errors}`, its images and
//! its search results.
//!
//! Reading is lenient where it costs nothing: a field that is missing or `null` takes its default,
//! fields not known are ignored, and a picture or game that cannot be read is left out rather
//! than failing the whole answer.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A whole answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Answer<T> {
    #[serde(default, deserialize_with = "or_default")]
    pub success: bool,
    pub data: Option<T>,
    #[serde(default, deserialize_with = "texts")]
    pub errors: Vec<String>,
}

/// One picture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Image {
    pub id: u64,
    #[serde(default, deserialize_with = "or_default")]
    pub score: i64,
    #[serde(default, deserialize_with = "or_default")]
    pub style: String,
    #[serde(default, deserialize_with = "or_default")]
    pub width: u32,
    #[serde(default, deserialize_with = "or_default")]
    pub height: u32,
    #[serde(default, deserialize_with = "or_default")]
    pub nsfw: bool,
    #[serde(default, deserialize_with = "or_default")]
    pub humor: bool,
    /// Whether it flashes.
    #[serde(default, deserialize_with = "or_default")]
    pub epilepsy: bool,
    #[serde(default, deserialize_with = "or_default")]
    pub mime: String,
    pub url: String,
    #[serde(default, deserialize_with = "or_default")]
    pub upvotes: i64,
}

/// A game a search found, or the game an id names.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Game {
    pub id: u64,
    #[serde(default, deserialize_with = "or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "or_default")]
    pub verified: bool,
}

/// Why an answer gives nothing to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerError {
    /// The body is not JSON of the API's shape, as the parser said it.
    Malformed(String),
    /// The API said it did not succeed, with the errors it gave.
    Unsuccessful(Vec<String>),
}

/// The pictures of an answer of `/grids`, `/heroes`, … for one id.
pub fn images(body: &[u8]) -> Result<Vec<Image>, AnswerError> {
    Ok(read_all(
        successful::<Vec<Value>>(body)?.unwrap_or_default(),
    ))
}

/// The game of an answer of `/games/<platform>/<id>`; `None` when it names none.
pub fn game(body: &[u8]) -> Result<Option<Game>, AnswerError> {
    Ok(successful::<Value>(body)?.and_then(|data| serde_json::from_value(data).ok()))
}

/// The games of an answer of `/search/autocomplete/<term>`, in the order given.
pub fn games(body: &[u8]) -> Result<Vec<Game>, AnswerError> {
    Ok(read_all(
        successful::<Vec<Value>>(body)?.unwrap_or_default(),
    ))
}

/// The errors an answer that failed gives, if the body is an answer at all.
pub fn errors(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<Answer<Value>>(body).map_or_default(|answer| answer.errors)
}

/// The data of a successful answer.
fn successful<T: DeserializeOwned>(body: &[u8]) -> Result<Option<T>, AnswerError> {
    let answer = serde_json::from_slice::<Answer<T>>(body)
        .map_err(|error| AnswerError::Malformed(error.to_string()))?;
    if answer.success {
        Ok(answer.data)
    } else {
        Err(AnswerError::Unsuccessful(answer.errors))
    }
}

/// Every item that reads as a `T`, in order.
fn read_all<T: DeserializeOwned>(items: Vec<Value>) -> Vec<T> {
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value(item).ok())
        .collect()
}

/// A value, or its default where it is `null`.
fn or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// A list of texts, from a list (whose other items are left out), one text, or `null`.
fn texts<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => vec![text],
        Value::Array(items) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    })
}

#[cfg(test)]
mod tests;
