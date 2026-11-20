use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    thread,
};

use anyhow::{Context, Result, ensure};
use fxtranslate::engine::Engine;
use tokio::sync::{mpsc, oneshot};

use crate::{
    Language,
    glossary::{Glossary, Terms},
};

type Direction = (Language, Language);

const DIRECT_MODELS: [Direction; 4] = [
    (Language::ZhCn, Language::En),
    (Language::En, Language::ZhCn),
    (Language::Ja, Language::En),
    (Language::En, Language::Ja),
];

pub(crate) enum ModelError {
    Busy,
    Stopped,
    Incomplete,
    Failed(anyhow::Error),
}

struct Job {
    from: Language,
    to: Language,
    text: String,
    response: oneshot::Sender<Result<Option<String>>>,
}

#[derive(Clone)]
pub(crate) struct LocalModel {
    jobs: mpsc::Sender<Job>,
}

impl LocalModel {
    pub(crate) fn start(directory: &Path, glossary_file: Option<&Path>) -> Result<Self> {
        let mut files = HashMap::new();
        for (from, to) in DIRECT_MODELS {
            let directory = directory.join(format!("{}-{}", from.code(), to.code()));
            let model = ModelFiles::inspect(&directory)?;
            files.insert((from, to), model);
        }
        let glossary = glossary_file.map(Glossary::load).transpose()?;

        // One active request and room for one queued request.
        let (jobs, mut receiver) = mpsc::channel::<Job>(1);
        thread::Builder::new()
            .name("sanyu-inference".into())
            .spawn(move || {
                let mut worker = ModelCache {
                    files,
                    loaded: None,
                };
                while let Some(job) = receiver.blocking_recv() {
                    if job.response.is_closed() {
                        continue;
                    }
                    let overrides = glossary
                        .as_ref()
                        .and_then(|rules| rules.get(job.from, job.to));
                    let result = worker.translate(job.from, job.to, &job.text, overrides);
                    let _ = job.response.send(result);
                }
            })
            .context("start local inference worker")?;
        Ok(Self { jobs })
    }

    pub(crate) async fn translate(
        &self,
        from: Language,
        to: Language,
        text: String,
    ) -> Result<String, ModelError> {
        let (response, receiver) = oneshot::channel();
        self.jobs
            .try_send(Job {
                from,
                to,
                text,
                response,
            })
            .map_err(|cause| match cause {
                mpsc::error::TrySendError::Full(_) => ModelError::Busy,
                mpsc::error::TrySendError::Closed(_) => ModelError::Stopped,
            })?;
        receiver
            .await
            .map_err(|_| ModelError::Stopped)?
            .map_err(ModelError::Failed)?
            .ok_or(ModelError::Incomplete)
    }
}

struct ModelFiles {
    weights: PathBuf,
    source_vocab: PathBuf,
    target_vocab: PathBuf,
}

impl ModelFiles {
    fn inspect(directory: &Path) -> Result<Self> {
        let weights = directory.join("model.bin");
        let shared = directory.join("vocab.spm");
        let (source_vocab, target_vocab) = if shared
            .try_exists()
            .with_context(|| format!("inspect model vocabulary {}", shared.display()))?
        {
            (shared.clone(), shared)
        } else {
            (directory.join("source.spm"), directory.join("target.spm"))
        };
        for path in [&weights, &source_vocab, &target_vocab] {
            let metadata = File::open(path)
                .and_then(|file| file.metadata())
                .with_context(|| format!("open model asset {}", path.display()))?;
            ensure!(
                metadata.is_file() && metadata.len() != 0,
                "model asset {} must be a nonempty file",
                path.display()
            );
        }
        Ok(Self {
            weights,
            source_vocab,
            target_vocab,
        })
    }
}

struct ModelCache {
    files: HashMap<Direction, ModelFiles>,
    loaded: Option<(Direction, Engine)>,
}

enum Fragment<'a> {
    Pending(Cow<'a, str>),
    Literal(&'a str),
}

impl ModelCache {
    fn translate(
        &mut self,
        from: Language,
        to: Language,
        source: &str,
        glossary: Option<&Terms>,
    ) -> Result<Option<String>> {
        let mut fragments = match glossary {
            Some(glossary) => split_glossary(source, from, glossary),
            None => vec![Fragment::Pending(Cow::Borrowed(source))],
        };

        let mut source_language = from;
        if from != Language::En && to != Language::En {
            if !self.translate_fragments((from, Language::En), &mut fragments)? {
                return Ok(None);
            }
            source_language = Language::En;
        }
        if !self.translate_fragments((source_language, to), &mut fragments)? {
            return Ok(None);
        }

        let mut output = String::new();
        for fragment in fragments {
            let text = match &fragment {
                Fragment::Pending(text) => text.as_ref(),
                Fragment::Literal(text) => text,
            };
            if to == Language::En
                && output.ends_with(char::is_alphanumeric)
                && text.starts_with(char::is_alphanumeric)
            {
                output.push(' ');
            }
            output.push_str(text);
        }
        Ok((!output.trim().is_empty()).then_some(output))
    }

    fn translate_fragments(
        &mut self,
        direction: Direction,
        fragments: &mut [Fragment<'_>],
    ) -> Result<bool> {
        let pending = fragments.iter_mut().filter_map(|fragment| match fragment {
            Fragment::Pending(text) => Some(text),
            Fragment::Literal(_) => None,
        });
        let mut pending = pending.peekable();
        if pending.peek().is_none() {
            return Ok(true);
        }
        let engine = self.engine(direction)?;
        for text in pending {
            let output = engine.translate_long(text);
            if incomplete(direction.1, text, &output) {
                return Ok(false);
            }
            *text = Cow::Owned(output);
        }
        Ok(true)
    }

    fn engine(&mut self, direction: Direction) -> Result<&Engine> {
        if self
            .loaded
            .as_ref()
            .is_none_or(|(current, _)| *current != direction)
        {
            // Release the previous model before opening the next one, including during a pivot.
            self.loaded = None;
            let files = self.files.get(&direction).context("missing direct model")?;
            let engine =
                Engine::load_mmapped(&files.weights, &files.source_vocab, &files.target_vocab)
                    .map_err(anyhow::Error::msg)
                    .with_context(|| format!("load model {}", files.weights.display()))?;
            self.loaded = Some((direction, engine));
        }
        Ok(&self.loaded.as_ref().context("model was not loaded")?.1)
    }
}

fn split_glossary<'a>(source: &'a str, from: Language, glossary: &'a Terms) -> Vec<Fragment<'a>> {
    let english = from == Language::En;
    let folded = english.then(|| source.to_ascii_lowercase());
    let lookup = folded.as_deref().unwrap_or(source);
    let mut fragments = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while cursor < source.len() {
        if let Some((end, translation)) = glossary.longest_match(lookup, cursor, english) {
            push_source(&mut fragments, &source[start..cursor]);
            fragments.push(Fragment::Literal(translation));
            cursor = end;
            start = end;
        } else {
            cursor += source[cursor..].chars().next().unwrap().len_utf8();
        }
    }
    push_source(&mut fragments, &source[start..]);
    fragments
}

fn push_source<'a>(fragments: &mut Vec<Fragment<'a>>, text: &'a str) {
    if !text.is_empty() {
        let fragment = if text.chars().any(char::is_alphabetic) {
            Fragment::Pending(Cow::Borrowed(text))
        } else {
            Fragment::Literal(text)
        };
        fragments.push(fragment);
    }
}

fn incomplete(target: Language, source: &str, output: &str) -> bool {
    let output = output.trim();
    if output.is_empty()
        || (output == source.trim()
            && source
                .chars()
                .filter(|c| c.is_alphabetic())
                .take(16)
                .count()
                == 16)
    {
        return true;
    }
    match target {
        Language::En => output.chars().any(|c| is_kana(c) || is_han(c)),
        Language::ZhCn => output.chars().any(is_kana),
        Language::Ja => false,
    }
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{3096}' | '\u{30a1}'..='\u{30fa}' | '\u{ff66}'..='\u{ff9d}')
}
