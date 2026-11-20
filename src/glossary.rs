use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, ensure};
use opencc_fmmseg::OpenCC;
use serde::Deserialize;

use crate::{Language, MAX_TEXT_CHARS};

type Entries = HashMap<String, String>;

pub(crate) struct Glossary {
    directions: HashMap<(Language, Language), Terms>,
}

pub(crate) struct Terms {
    entries: Entries,
    byte_lengths: Vec<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GlossaryEntry {
    from: Language,
    to: Language,
    text: String,
    translation: String,
}

impl Glossary {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        Self::read(path).with_context(|| format!("load glossary {}", path.display()))
    }

    fn read(path: &Path) -> Result<Self> {
        let entries: Vec<GlossaryEntry> =
            serde_json::from_slice(&fs::read(path)?).context("parse glossary JSON array")?;
        let opencc = OpenCC::new();
        let mut tables = HashMap::<_, Entries>::new();
        for (index, entry) in entries.into_iter().enumerate() {
            let number = index + 1;
            ensure!(
                entry.from != entry.to,
                "glossary entry {number}: from and to must be different"
            );
            ensure!(
                !entry.text.trim().is_empty() && !entry.text.contains(['\r', '\n']),
                "glossary entry {number}: text must be a nonblank single line"
            );
            ensure!(
                entry.text.chars().take(MAX_TEXT_CHARS + 1).count() <= MAX_TEXT_CHARS,
                "glossary entry {number}: text exceeds {MAX_TEXT_CHARS} Unicode characters"
            );
            ensure!(
                !entry.translation.trim().is_empty(),
                "glossary entry {number}: translation must not be blank"
            );
            let source = match entry.from {
                Language::En => entry.text.to_ascii_lowercase(),
                Language::ZhCn => opencc.t2s(&entry.text, false),
                Language::Ja => entry.text,
            };
            let target = if entry.to == Language::ZhCn {
                opencc.t2s(&entry.translation, false)
            } else {
                entry.translation
            };
            let table = tables.entry((entry.from, entry.to)).or_default();
            ensure!(
                table.insert(source, target).is_none(),
                "glossary entry {number}: duplicate source text in this direction"
            );
        }

        let directions = tables
            .into_iter()
            .map(|(pair, entries)| (pair, Terms::new(entries)))
            .collect();
        Ok(Self { directions })
    }

    pub(crate) fn get(&self, from: Language, to: Language) -> Option<&Terms> {
        self.directions.get(&(from, to))
    }
}

impl Terms {
    fn new(entries: Entries) -> Self {
        let lengths: HashSet<_> = entries.keys().map(String::len).collect();
        let mut byte_lengths: Vec<_> = lengths.into_iter().collect();
        byte_lengths.sort_unstable_by(|left, right| right.cmp(left));
        Self {
            entries,
            byte_lengths,
        }
    }

    pub(crate) fn longest_match(
        &self,
        line: &str,
        start: usize,
        english: bool,
    ) -> Option<(usize, &str)> {
        if english && !is_word_boundary(line, start) {
            return None;
        }

        let remaining = &line[start..];
        let first_length = self
            .byte_lengths
            .partition_point(|&length| length > remaining.len());
        for &length in &self.byte_lengths[first_length..] {
            let Some(candidate) = remaining.get(..length) else {
                continue;
            };
            let Some(translation) = self.entries.get(candidate) else {
                continue;
            };
            let end = start + length;
            if !english || is_word_boundary(line, end) {
                return Some((end, translation));
            }
        }
        None
    }
}

fn is_word_boundary(line: &str, offset: usize) -> bool {
    let mut before = line[..offset].chars().rev();
    let mut after = line[offset..].chars();
    let (Some(left), Some(right)) = (before.next(), after.next()) else {
        return true;
    };
    // Apostrophes connect letters inside words while quoted words have boundaries.
    match (left, right) {
        (left, '\'' | '’') if is_word_char(left) => !after.next().is_some_and(is_word_char),
        ('\'' | '’', right) if is_word_char(right) => !before.next().is_some_and(is_word_char),
        (left, right) => !(is_word_char(left) && is_word_char(right)),
    }
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}
