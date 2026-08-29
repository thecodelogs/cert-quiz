use std::collections::BTreeMap;

pub const SET_LENGTH: usize = 20;
pub const LETTERS: [char; 6] = ['A', 'B', 'C', 'D', 'E', 'F'];

#[derive(Debug, Clone)]
pub struct Question {
    pub q: String,
    pub options: Vec<String>,
    pub answer: Vec<usize>,
    pub domain: String,
    pub explain: String,
}

#[derive(Debug, Clone)]
pub struct Cert {
    pub id: String,
    pub code: String,
    pub pass_mark: u32,
    pub questions: Vec<Question>,
}

impl Cert {
    /// The questions in one attempt. Shuffle or query a random set here later.
    pub fn set(&self) -> &[Question] {
        let n = self.questions.len().min(SET_LENGTH);
        &self.questions[..n]
    }
}

pub fn is_correct(question: &Question, picked: &[usize]) -> bool {
    if picked.len() != question.answer.len() {
        return false;
    }
    let mut a = picked.to_vec();
    let mut b = question.answer.clone();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

pub fn fmt_clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub fn join_answer(question: &Question, picks: &[usize]) -> String {
    if picks.is_empty() {
        return "—".to_string();
    }
    let mut sorted = picks.to_vec();
    sorted.sort_unstable();
    sorted
        .iter()
        .map(|&i| format!("{}. {}", LETTERS[i], question.options[i]))
        .collect::<Vec<_>>()
        .join("  ·  ")
}

/// Answers for one attempt: question index -> chosen option indices.
pub type Answers = BTreeMap<usize, Vec<usize>>;

pub struct DomainScore {
    pub name: String,
    pub ok: usize,
    pub n: usize,
    pub pct: u32,
}

pub struct ReviewRow {
    pub num: String,
    pub question: String,
    pub status: &'static str,
    pub ok: bool,
    pub yours: String,
    pub correct: String,
}

pub struct Score {
    pub total: usize,
    pub correct: usize,
    pub wrong: usize,
    pub skipped: usize,
    pub pct: u32,
    pub domains: Vec<DomainScore>,
    pub review: Vec<ReviewRow>,
}

pub fn score(cert: &Cert, answers: &Answers) -> Score {
    let qs = cert.set();
    let mut correct = 0usize;
    let mut skipped = 0usize;
    let mut domains: Vec<DomainScore> = Vec::new();
    let mut review = Vec::new();

    for (i, q) in qs.iter().enumerate() {
        let empty: Vec<usize> = Vec::new();
        let picked = answers.get(&i).unwrap_or(&empty);
        let ok = is_correct(q, picked);
        if ok {
            correct += 1;
        }
        if picked.is_empty() {
            skipped += 1;
        }

        match domains.iter_mut().find(|d| d.name == q.domain) {
            Some(d) => {
                d.n += 1;
                if ok {
                    d.ok += 1;
                }
            }
            None => domains.push(DomainScore {
                name: q.domain.clone(),
                ok: if ok { 1 } else { 0 },
                n: 1,
                pct: 0,
            }),
        }

        review.push(ReviewRow {
            num: format!("{:02}", i + 1),
            question: q.q.clone(),
            status: if picked.is_empty() {
                "Skipped"
            } else if ok {
                "Correct"
            } else {
                "Incorrect"
            },
            ok,
            yours: join_answer(q, picked),
            correct: join_answer(q, &q.answer),
        });
    }

    for d in domains.iter_mut() {
        d.pct = ((d.ok as f32 / d.n as f32) * 100.0).round() as u32;
    }

    Score {
        total: qs.len(),
        correct,
        wrong: qs.len() - correct - skipped,
        skipped,
        pct: ((correct as f32 / qs.len() as f32) * 100.0).round() as u32,
        domains,
        review,
    }
}
