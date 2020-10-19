use rand::distributions::{WeightedError, WeightedIndex};
use rand::{distributions::Distribution, Rng};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::{Entry as HashEntry, HashMap};
use std::collections::HashSet;
use std::convert::TryFrom;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone)]
pub enum Word {
    Start,
    End,
    Word(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(try_from = "HashMap<Word, usize>")]
#[serde(into = "HashMap<Word, usize>")]
struct Entry {
    weight_pairs: Vec<(Word, usize)>,
    dist: WeightedIndex<usize>,
}

impl Entry {
    fn new(word: Word) -> Self {
        Entry {
            weight_pairs: vec![(word, 1)],
            dist: WeightedIndex::new(std::iter::once(1))
                .expect("should create value weighted index"),
        }
    }

    fn get_random(&self, rng: &mut impl Rng) -> Word {
        self.weight_pairs[self.dist.sample(rng)].0.clone()
    }

    fn insert(&mut self, new_word: Word) {
        for (i, pair) in self.weight_pairs.iter_mut().enumerate() {
            let (word, weight) = pair;
            if *word == new_word {
                *weight += 1;
                self.dist
                    .update_weights(&[(i, weight)])
                    .expect("update should keep valid weights");
                return;
            }
        }
        self.weight_pairs.push((new_word, 1));
        self.dist = self
            .gen_new_weights()
            .expect("dist with added word should be valid");
    }

    fn clean(mut self, threshold: usize) -> (Option<Self>, usize) {
        let old_size = self.weight_pairs.len();
        self.weight_pairs.retain(|(_, w)| *w >= threshold);
        self.dist = match self.gen_new_weights() {
            Ok(d) => d,
            Err(_) => return (None, old_size),
        };
        let removed = old_size - self.weight_pairs.len();
        (Some(self), removed)
    }

    fn gen_new_weights(&self) -> Result<WeightedIndex<usize>, WeightedError> {
        WeightedIndex::new(self.weight_pairs.iter().map(|(_, w)| *w))
    }
}

impl TryFrom<HashMap<Word, usize>> for Entry {
    type Error = WeightedError;

    fn try_from(map: HashMap<Word, usize>) -> Result<Self, Self::Error> {
        let mut weight_pairs = Vec::with_capacity(map.len());
        let dist = WeightedIndex::new(map.into_iter().map(|(w, s)| {
            weight_pairs.push((w, s));
            s
        }))?;
        Ok(Entry { weight_pairs, dist })
    }
}

impl From<Entry> for HashMap<Word, usize> {
    fn from(entry: Entry) -> Self {
        entry.weight_pairs.into_iter().collect()
    }
}

pub const WORD_COUNT: usize = 2;
pub type WordArray = [Word; WORD_COUNT];
pub const START_WORDS: WordArray = [Word::Start, Word::Start];

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Markov {
    entries: HashMap<WordArray, Entry>,
}

impl Markov {
    pub fn new() -> Self {
        Markov {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, index: WordArray, word: Word) {
        match self.entries.entry(index) {
            HashEntry::Occupied(mut e) => {
                e.get_mut().insert(word);
            }
            HashEntry::Vacant(e) => {
                e.insert(Entry::new(word));
            }
        }
    }

    pub fn insert_sequence(&mut self, seq: impl IntoIterator<Item = String>) {
        let mut prevs = (Word::Start, Word::Start);
        for cur in seq {
            let cur = Word::Word(cur);
            self.insert([prevs.0, prevs.1.clone()], cur.clone());
            prevs.0 = std::mem::replace(&mut prevs.1, cur);
        }
        self.insert([prevs.0, prevs.1], Word::End);
    }

    pub fn generate_sequence<R: Rng>(&self, rng: R) -> Chain<'_, R> {
        Chain {
            entries: &self.entries,
            cur_words: START_WORDS,
            rng,
        }
    }

    pub fn clean(&mut self, threshold: usize) -> usize {
        let mut sum = 0;
        self.entries = self
            .entries
            .drain()
            .filter_map(|(k, e)| {
                let (cleaned, removed) = e.clean(threshold);
                sum += removed;
                cleaned.map(|c| (k, c))
            })
            .collect();
        sum
    }

    pub fn what_follows(&self, word: &str) -> HashSet<String> {
        let word = Word::Word(word.into());
        self.entries
            .iter()
            .filter_map(|([_, snd], e)| if *snd == word { Some(e) } else { None })
            .flat_map(|e| {
                e.weight_pairs.iter().filter_map(|(word, _)| match word {
                    Word::Word(w) => Some(w.clone()),
                    _ => None,
                })
            })
            .collect()
    }
}

pub struct Chain<'a, R> {
    entries: &'a HashMap<WordArray, Entry>,
    cur_words: WordArray,
    rng: R,
}

impl<R: Rng> Iterator for Chain<'_, R> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let cur_entry = self.entries.get(&self.cur_words)?;
        let word = cur_entry.get_random(&mut self.rng);
        eprintln!("got {:?} looking after {:?}", word, self.cur_words);
        self.cur_words[0] = std::mem::replace(&mut self.cur_words[1], Word::End);
        self.cur_words[1] = word.clone();
        match word {
            Word::Word(w) => Some(w),
            Word::End => None,
            Word::Start => unreachable!(),
        }
    }
}
