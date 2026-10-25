//! Romaji for Japanese lyrics lines in modified Hepburn (docs/romaji.md). Lindera with the
//! embedded IPADIC splits a line into morphemes and gives each its kana reading and its
//! pronunciation; the rules here join morphemes into words the way romanised lyrics are usually
//! written and spell the kana in Hepburn. Runs once per song, when the lyrics arrive or the
//! setting is turned on, never per frame.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use unicode_normalization::UnicodeNormalization;

use super::types::Line;
use super::words;

/// Kana or kanji in the text; the same test as `langOf` in the overlay (lyricsTimeline.ts).
pub fn is_japanese(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ff66}'..='\u{ff9f}')
    })
}

/// Adds romaji to every line with kana or kanji that has none yet. Repeated lines (a chorus)
/// are converted once. Returns how many lines got romaji.
pub fn annotate(lines: &mut [Line]) -> usize {
    if !lines.iter().any(|l| l.romaji.is_none() && is_japanese(&l.text)) {
        return 0;
    }
    let Some(romanizer) = shared() else { return 0 };
    let mut memo: HashMap<String, Option<String>> = HashMap::new();
    let mut count = 0;
    for line in lines.iter_mut().filter(|l| l.romaji.is_none() && is_japanese(&l.text)) {
        let romaji = memo.entry(line.text.clone()).or_insert_with(|| romanizer.line(&line.text)).clone();
        count += usize::from(romaji.is_some());
        line.romaji_words = romaji.as_deref().map(|r| words::estimate(r, line.start_ms, line.fill_end_ms)).filter(|w| !w.is_empty());
        line.romaji = romaji;
    }
    count
}

/// The dictionary loads on first use (under 1 ms, it is mapped from the binary) and stays.
fn shared() -> Option<&'static Romanizer> {
    static SHARED: OnceLock<Option<Romanizer>> = OnceLock::new();
    SHARED
        .get_or_init(|| match Romanizer::new() {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!(error = %e, "romaji dictionary unavailable");
                None
            }
        })
        .as_ref()
}

pub struct Romanizer {
    segmenter: Segmenter,
}

impl Romanizer {
    pub fn new() -> Result<Self, String> {
        let dictionary = load_dictionary("embedded://ipadic").map_err(|e| e.to_string())?;
        Ok(Self { segmenter: Segmenter::new(Mode::Normal, dictionary, None) })
    }

    /// Romaji of a line with kana or kanji; `None` for other lines.
    pub fn line(&self, text: &str) -> Option<String> {
        if !is_japanese(text) {
            return None;
        }
        // NFKC: half-width katakana become full-width, which the dictionary knows; full-width
        // letters, digits, marks and spaces become ASCII.
        let text: String = text.nfkc().collect();
        let mut out = Output::default();
        for chunk in text.split_whitespace() {
            if is_japanese(chunk) {
                romanize(&self.morphemes(chunk)?, &mut out);
            } else {
                out.word(chunk, false);
            }
        }
        Some(out.text)
    }

    fn morphemes(&self, chunk: &str) -> Option<Vec<Morpheme>> {
        let mut tokens = self.segmenter.segment(Cow::Borrowed(chunk)).ok()?;
        Some(
            tokens
                .iter_mut()
                .map(|t| {
                    let surface = t.surface.to_string();
                    Morpheme::new(surface, &t.details())
                })
                .collect(),
        )
    }
}

/// One IPADIC token. Reading and pronunciation are katakana; unknown words have neither.
struct Morpheme {
    surface: String,
    pos: String,
    pos1: String,
    conj: String,
    base: String,
    reading: Option<String>,
    pron: Option<String>,
}

impl Morpheme {
    /// IPADIC details: part of speech and three subclasses, conjugation type and form, base
    /// form, reading, pronunciation.
    fn new(surface: String, details: &[&str]) -> Self {
        let field = |i: usize| details.get(i).filter(|v| !v.is_empty() && **v != "*").map(|v| v.to_string());
        Self {
            surface,
            pos: field(0).unwrap_or_default(),
            pos1: field(1).unwrap_or_default(),
            conj: field(4).unwrap_or_default(),
            base: field(6).unwrap_or_default(),
            reading: field(7),
            pron: field(8),
        }
    }

    fn is_japanese(&self) -> bool {
        is_japanese(&self.surface)
    }
}

/// Groups the morphemes of one chunk (text between spaces) into words and appends them.
fn romanize(morphemes: &[Morpheme], out: &mut Output) {
    let mut word: Option<Word> = None;
    let mut prev: Option<&Morpheme> = None;
    for m in morphemes {
        if let Some(mark) = punctuation(&m.surface) {
            flush(&mut word, out);
            out.mark(mark);
            prev = None;
            continue;
        }
        if !m.is_japanese() {
            // Latin letters or digits between Japanese words stay as written.
            let glue = prev.is_some_and(|p| !p.is_japanese());
            flush(&mut word, out);
            out.word(&m.surface, glue);
            prev = Some(m);
            continue;
        }
        if !(word.is_some() && prev.is_some_and(|p| attaches(p, m))) {
            flush(&mut word, out);
        }
        // Proper nouns start with a capital; unknown words have no reading and stay lower case.
        let w = word.get_or_insert_with(|| Word { parts: Vec::new(), proper: m.pos1 == "固有名詞" && m.reading.is_some() });
        match kana_of(m) {
            // The volitional う after an o (行こう, でしょう) is a long ō in Hepburn.
            Some(_) if m.pos == "助動詞" && m.surface == "う" && w.last_vowel() == Some('o') => w.push_kana("ー"),
            Some(kana) => w.push_kana(&kana),
            None => w.parts.push(Part::Raw(m.surface.clone())),
        }
        prev = Some(m);
    }
    flush(&mut word, out);
}

/// Whether `m` continues the word of `prev`, as romanised lyrics usually write it: conjugation
/// endings and auxiliaries join the verb (kimashita, hashitte, matteru), particles, the copula
/// and helper verbs stand alone (kimi no, suki da, hashitte iru).
fn attaches(prev: &Morpheme, m: &Morpheme) -> bool {
    if prev.pos == "接頭詞" {
        return true; // お + 兄さん → onīsan
    }
    match (m.pos.as_str(), m.pos1.as_str()) {
        // The copula stands alone (suki da), the past tense だ does not (yonda): 特殊・タ.
        ("助動詞", _) => !matches!(m.base.as_str(), "だ" | "です") || m.conj == "特殊・タ",
        ("助詞", "接続助詞") => matches!(m.surface.as_str(), "て" | "で" | "ば" | "ちゃ" | "じゃ"),
        ("動詞", "接尾") | ("名詞", "接尾") | ("形容詞", "接尾") => true,
        ("動詞", "非自立") => matches!(m.base.as_str(), "てる" | "でる" | "ちゃう" | "じゃう" | "とく" | "どく"),
        ("名詞", "非自立") => m.surface == "ん",
        _ => false,
    }
}

/// Katakana of a morpheme as Hepburn spells it, or `None` for an unknown word in kanji.
fn kana_of(m: &Morpheme) -> Option<String> {
    match (&m.reading, &m.pron) {
        (Some(reading), Some(pron)) => Some(spoken(reading, pron)),
        (Some(reading), None) => Some(reading.clone()),
        _ if m.surface.chars().all(|c| matches!(c, '\u{3041}'..='\u{30ff}')) => Some(m.surface.chars().map(to_katakana).collect()),
        _ => None,
    }
}

/// The reading with the long vowels of the pronunciation (トウキョウ, トーキョー → トーキョー)
/// and its wa, e, o for は, へ, を. Elsewhere the reading wins, because Hepburn spells words as
/// written: 先生 stays sensei, 小さい chiisai, 言う iu.
fn spoken(reading: &str, pron: &str) -> String {
    let (r, p): (Vec<char>, Vec<char>) = (reading.chars().collect(), pron.chars().collect());
    if r.len() != p.len() {
        return reading.to_owned();
    }
    let mut out = String::with_capacity(reading.len());
    let mut vowel = None;
    for (&rc, &pc) in r.iter().zip(&p) {
        let c = match (rc, pc) {
            _ if rc == pc => rc,
            ('イ', 'ー') if matches!(vowel, Some('e' | 'i')) => 'イ',
            (_, 'ー') => 'ー',
            ('ハ', 'ワ') | ('ヘ', 'エ') | ('ヲ', 'オ') => pc,
            _ => rc,
        };
        if c != 'ー' {
            vowel = vowel_of(c);
        }
        out.push(c);
    }
    out
}

/// Kana (katakana or hiragana) in modified Hepburn: ッ doubles the next consonant (ッチ → tchi),
/// ン before a vowel or y is n', ー lengthens the vowel before it with a macron.
pub fn hepburn(kana: &str) -> String {
    let chars: Vec<char> = kana.chars().map(to_katakana).collect();
    let mut out = String::with_capacity(chars.len() * 2);
    let mut sokuon = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            'ッ' => {
                sokuon = true;
                continue;
            }
            'ー' => lengthen(&mut out),
            'ン' => {
                out.push('n');
                // kin'en and kon'ya stay apart from ki-nen and ko-nya.
                if syllable(&chars[i..]).is_some_and(|(s, _)| s.starts_with(['a', 'i', 'u', 'e', 'o', 'y'])) {
                    out.push('\'');
                }
            }
            _ => match syllable(&chars[i - 1..]) {
                Some((s, used)) => {
                    if sokuon {
                        if s.starts_with("ch") {
                            out.push('t');
                        } else if let Some(first) = s.chars().next().filter(|f| !"aiueo".contains(*f)) {
                            out.push(first);
                        }
                    }
                    out.push_str(&s);
                    i += used - 1;
                }
                None => out.push(c),
            },
        }
        sokuon = false;
    }
    out
}

/// The syllable at the start of `chars` and how many kana it takes.
fn syllable(chars: &[char]) -> Option<(Cow<'static, str>, usize)> {
    let first = *chars.first()?;
    if let Some(&second) = chars.get(1)
        && let Some(s) = digraph(first, second)
    {
        return Some((s, 2));
    }
    single(first).map(|s| (Cow::Borrowed(s), 1))
}

fn digraph(a: char, b: char) -> Option<Cow<'static, str>> {
    // Yōon: キャ kya, シャ sha, チュ chu, ジョ jo, …
    if let Some(vowel) = match b {
        'ャ' => Some('a'),
        'ュ' => Some('u'),
        'ョ' => Some('o'),
        _ => None,
    } {
        let onset = match a {
            'キ' => "ky",
            'ギ' => "gy",
            'シ' => "sh",
            'ジ' | 'ヂ' => "j",
            'チ' => "ch",
            'ニ' => "ny",
            'ヒ' => "hy",
            'ビ' => "by",
            'ピ' => "py",
            'ミ' => "my",
            'リ' => "ry",
            'テ' => "ty",
            'デ' => "dy",
            'フ' => "fy",
            'ヴ' => "vy",
            _ => return None,
        };
        return Some(Cow::Owned(format!("{onset}{vowel}")));
    }
    // Small vowels, mostly in loanwords: ティ ti, ファ fa, ウェ we, …
    let s = match (a, b) {
        ('ウ', 'ィ') => "wi",
        ('ウ', 'ェ') => "we",
        ('ウ', 'ォ') => "wo",
        ('ヴ', 'ァ') => "va",
        ('ヴ', 'ィ') => "vi",
        ('ヴ', 'ェ') => "ve",
        ('ヴ', 'ォ') => "vo",
        ('シ', 'ェ') => "she",
        ('ジ', 'ェ') => "je",
        ('チ', 'ェ') => "che",
        ('テ', 'ィ') => "ti",
        ('デ', 'ィ') => "di",
        ('ト', 'ゥ') => "tu",
        ('ド', 'ゥ') => "du",
        ('ツ', 'ァ') => "tsa",
        ('ツ', 'ィ') => "tsi",
        ('ツ', 'ェ') => "tse",
        ('ツ', 'ォ') => "tso",
        ('フ', 'ァ') => "fa",
        ('フ', 'ィ') => "fi",
        ('フ', 'ェ') => "fe",
        ('フ', 'ォ') => "fo",
        ('イ', 'ェ') => "ye",
        ('ク', 'ァ') => "kwa",
        ('グ', 'ァ') => "gwa",
        _ => return None,
    };
    Some(Cow::Borrowed(s))
}

#[rustfmt::skip]
fn single(c: char) -> Option<&'static str> {
    Some(match c {
        'ア' => "a", 'イ' => "i", 'ウ' => "u", 'エ' => "e", 'オ' => "o",
        'カ' => "ka", 'キ' => "ki", 'ク' => "ku", 'ケ' => "ke", 'コ' => "ko",
        'ガ' => "ga", 'ギ' => "gi", 'グ' => "gu", 'ゲ' => "ge", 'ゴ' => "go",
        'サ' => "sa", 'シ' => "shi", 'ス' => "su", 'セ' => "se", 'ソ' => "so",
        'ザ' => "za", 'ジ' => "ji", 'ズ' => "zu", 'ゼ' => "ze", 'ゾ' => "zo",
        'タ' => "ta", 'チ' => "chi", 'ツ' => "tsu", 'テ' => "te", 'ト' => "to",
        'ダ' => "da", 'ヂ' => "ji", 'ヅ' => "zu", 'デ' => "de", 'ド' => "do",
        'ナ' => "na", 'ニ' => "ni", 'ヌ' => "nu", 'ネ' => "ne", 'ノ' => "no",
        'ハ' => "ha", 'ヒ' => "hi", 'フ' => "fu", 'ヘ' => "he", 'ホ' => "ho",
        'バ' => "ba", 'ビ' => "bi", 'ブ' => "bu", 'ベ' => "be", 'ボ' => "bo",
        'パ' => "pa", 'ピ' => "pi", 'プ' => "pu", 'ペ' => "pe", 'ポ' => "po",
        'マ' => "ma", 'ミ' => "mi", 'ム' => "mu", 'メ' => "me", 'モ' => "mo",
        'ヤ' => "ya", 'ユ' => "yu", 'ヨ' => "yo",
        'ラ' => "ra", 'リ' => "ri", 'ル' => "ru", 'レ' => "re", 'ロ' => "ro",
        'ワ' => "wa", 'ヰ' => "i", 'ヱ' => "e", 'ヲ' => "o", 'ン' => "n", 'ヴ' => "vu",
        'ァ' => "a", 'ィ' => "i", 'ゥ' => "u", 'ェ' => "e", 'ォ' => "o",
        'ャ' => "ya", 'ュ' => "yu", 'ョ' => "yo", 'ヮ' => "wa", 'ヵ' => "ka", 'ヶ' => "ke",
        _ => return None,
    })
}

fn vowel_of(kana: char) -> Option<char> {
    single(kana).and_then(|s| s.chars().last()).filter(|v| "aiueo".contains(*v))
}

fn lengthen(out: &mut String) {
    let long = match out.chars().last() {
        Some('a') => 'ā',
        Some('i') => 'ī',
        Some('u') => 'ū',
        Some('e') => 'ē',
        Some('o') => 'ō',
        _ => return,
    };
    out.pop();
    out.push(long);
}

fn to_katakana(c: char) -> char {
    match c {
        '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
        _ => c,
    }
}

/// A word in the making: kana to spell in Hepburn, or text kept as written.
struct Word {
    parts: Vec<Part>,
    proper: bool,
}

enum Part {
    Kana(String),
    Raw(String),
}

impl Word {
    fn push_kana(&mut self, kana: &str) {
        match self.parts.last_mut() {
            Some(Part::Kana(k)) => k.push_str(kana),
            _ => self.parts.push(Part::Kana(kana.to_owned())),
        }
    }

    fn last_vowel(&self) -> Option<char> {
        match self.parts.last() {
            Some(Part::Kana(k)) => k.chars().last().and_then(vowel_of),
            _ => None,
        }
    }

    fn romaji(&self) -> String {
        // Kana parts are joined first, so っ at the end of one morpheme doubles the next (hashitte).
        let mut s = String::new();
        for part in &self.parts {
            match part {
                Part::Kana(k) => s.push_str(&hepburn(k)),
                Part::Raw(r) => s.push_str(r),
            }
        }
        if !self.proper {
            return s;
        }
        let mut chars = s.chars();
        chars.next().map(|f| f.to_uppercase().chain(chars).collect()).unwrap_or_default()
    }
}

fn flush(word: &mut Option<Word>, out: &mut Output) {
    if let Some(w) = word.take() {
        out.word(&w.romaji(), false);
    }
}

enum Mark {
    /// Binds to the next word: an opening quote or bracket.
    Open(&'static str),
    /// Binds to the word before: comma, full stop, closing quote, !, ?, …
    Close(String),
    /// Separates words without a sign of its own (・).
    Gap,
}

/// Punctuation and symbols (after NFKC), with their romaji form.
fn punctuation(surface: &str) -> Option<Mark> {
    if surface.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(match surface {
        "、" => Mark::Close(",".into()),
        "。" => Mark::Close(".".into()),
        "「" | "『" => Mark::Open("\""),
        "」" | "』" => Mark::Close("\"".into()),
        "(" => Mark::Open("("),
        "【" | "[" => Mark::Open("["),
        "】" => Mark::Close("]".into()),
        "〜" => Mark::Close("~".into()),
        "・" => Mark::Gap,
        other => Mark::Close(other.to_owned()),
    })
}

#[derive(Default)]
struct Output {
    text: String,
    /// The last thing written was an opening mark, so the next word follows without a space.
    glue_next: bool,
}

impl Output {
    fn word(&mut self, word: &str, glue: bool) {
        if word.is_empty() {
            return;
        }
        if !self.text.is_empty() && !glue && !self.glue_next {
            self.text.push(' ');
        }
        self.text.push_str(word);
        self.glue_next = false;
    }

    fn mark(&mut self, mark: Mark) {
        match mark {
            Mark::Open(s) => {
                if !self.text.is_empty() && !self.glue_next {
                    self.text.push(' ');
                }
                self.text.push_str(s);
                self.glue_next = true;
            }
            Mark::Close(s) => {
                self.text.push_str(&s);
                self.glue_next = false;
            }
            Mark::Gap => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invented sentences only, never lyrics (PLAN.md 15, docs/phase2.md).

    fn romaji(text: &str) -> String {
        Romanizer::new().unwrap().line(text).unwrap_or_else(|| panic!("no romaji for {text}"))
    }

    #[test]
    fn kana_in_modified_hepburn() {
        let cases = [
            ("しんぶん", "shinbun"),
            ("キンエン", "kin'en"),
            ("こんや", "kon'ya"),
            ("マッチ", "matchi"),
            ("ガッコー", "gakkō"),
            ("ちゃ じゅ しょ", "cha ju sho"),
            ("ティーカップ", "tīkappu"),
            ("ヴァイオリン", "vaiorin"),
            ("ファン", "fan"),
            ("つづく", "tsuzuku"),
            ("ヂャ", "ja"),
            ("ン", "n"),
            ("あっ", "a"),
        ];
        for (kana, expected) in cases {
            assert_eq!(hepburn(kana), expected, "{kana}");
        }
    }

    #[test]
    fn pronunciation_only_for_long_vowels_and_particles() {
        assert_eq!(spoken("トウキョウ", "トーキョー"), "トーキョー");
        assert_eq!(spoken("オカアサン", "オカーサン"), "オカーサン");
        assert_eq!(spoken("センセイ", "センセー"), "センセイ");
        assert_eq!(spoken("チイサイ", "チーサイ"), "チイサイ");
        assert_eq!(spoken("イウ", "ユウ"), "イウ");
        assert_eq!(spoken("ハ", "ワ"), "ワ");
        assert_eq!(spoken("コンニチハ", "コンニチワ"), "コンニチワ");
        assert_eq!(spoken("ア", "アー"), "ア");
    }

    #[test]
    fn particles_long_vowels_and_proper_nouns() {
        assert_eq!(romaji("東京の空は青いと思う"), "Tōkyō no sora wa aoi to omou");
        assert_eq!(romaji("ありがとう、そして先生へ"), "arigatō, soshite sensei e");
        assert_eq!(romaji("スーパーでコーヒーを買う"), "sūpā de kōhī o kau");
        assert_eq!(romaji("お母さんとお兄さんが来ました"), "okāsan to oniisan ga kimashita");
    }

    #[test]
    fn verb_endings_join_the_verb() {
        assert_eq!(romaji("君の声が聞こえた夜に"), "kimi no koe ga kikoeta yoru ni");
        assert_eq!(romaji("僕らはまだ走っている"), "bokura wa mada hashitte iru");
        assert_eq!(romaji("ずっと待ってる"), "zutto matteru");
        assert_eq!(romaji("言わないでって言ったのに"), "iwanaide tte itta noni");
        assert_eq!(romaji("新しい世界へ行こう！"), "atarashii sekai e ikō!");
        assert_eq!(romaji("きっと明日は晴れるでしょう"), "kitto ashita wa hareru deshō");
        assert_eq!(romaji("繋いだ手は静かだった"), "tsunaida te wa shizuka datta");
        assert_eq!(romaji("もう行っちゃうだろう"), "mō itchau darō");
    }

    #[test]
    fn mixed_scripts_and_widths() {
        assert_eq!(romaji("Baby 君が好きだよ"), "Baby kimi ga suki da yo");
        assert_eq!(romaji("ｶﾀｶﾅ"), "katakana");
        assert_eq!(romaji("１２３　テスト"), "123 tesuto");
        assert_eq!(romaji("「夢」を見た"), "\"yume\" o mita");
    }

    #[test]
    fn other_lines_stay_as_they_are() {
        let r = Romanizer::new().unwrap();
        assert_eq!(r.line("Only English here"), None);
        assert_eq!(r.line(""), None);
        assert!(is_japanese("ｱ") && is_japanese("漢") && !is_japanese("Ünïcödé"));
    }

    #[test]
    fn annotate_fills_japanese_lines_once() {
        let line = |text: &str| Line { start_ms: 0, end_ms: None, fill_end_ms: 0, text: text.into(), words: None, romaji: None, romaji_words: None };
        let mut lines = vec![line("夢を見た"), line("Plain text"), line(""), line("夢を見た")];
        assert_eq!(annotate(&mut lines), 2);
        let got: Vec<Option<&str>> = lines.iter().map(|l| l.romaji.as_deref()).collect();
        assert_eq!(got, [Some("yume o mita"), None, None, Some("yume o mita")]);
        assert_eq!(annotate(&mut lines), 0);
    }

    #[test]
    fn overlay_sample_line() {
        // Shown in the edit mode and the settings preview (Overlay.svelte, SAMPLE_NEXT_ROMAJI).
        assert_eq!(romaji("次の歌詞はここに少し小さく表示されます"), "tsugi no kashi wa koko ni sukoshi chiisaku hyōji saremasu");
    }
}
