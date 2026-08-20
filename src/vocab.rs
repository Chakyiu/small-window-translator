use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Word {
    pub id: i64,
    pub word: String,
    pub translation: String,
    pub source_lang: String,
    pub target_lang: String,
    pub provider: String,
    #[allow(dead_code)]
    pub created_at: i64,
    pub last_reviewed_at: Option<i64>,
    pub review_count: i64,
}

#[derive(Debug, Clone)]
pub struct NewWord {
    pub word: String,
    pub translation: String,
    pub source_lang: String,
    pub target_lang: String,
    pub provider: String,
}

pub fn db_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("dev", "swtrans", "swtrans")
        .context("could not resolve data directory")?;
    Ok(dirs.data_dir().join("vocab.db"))
}

pub fn is_saved(word: &str, source_lang: &str, target_lang: &str) -> bool {
    match db_path().and_then(connect) {
        Ok(conn) => find(&conn, word, source_lang, target_lang)
            .ok()
            .flatten()
            .is_some(),
        Err(_) => false,
    }
}

pub fn list() -> Result<Vec<Word>> {
    list_at(&db_path()?)
}

pub fn toggle(entry: NewWord) -> Result<bool> {
    toggle_at(&db_path()?, entry)
}

pub fn delete(id: i64) -> Result<()> {
    delete_at(&db_path()?, id)
}

pub fn mark_reviewed(id: i64) -> Result<()> {
    mark_reviewed_at(&db_path()?, id)
}

fn connect(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path).context("open vocab.db")?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS words (
            id INTEGER PRIMARY KEY,
            word TEXT NOT NULL,
            translation TEXT NOT NULL DEFAULT '',
            source_lang TEXT NOT NULL DEFAULT '',
            target_lang TEXT NOT NULL DEFAULT '',
            provider TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            last_reviewed_at INTEGER,
            review_count INTEGER NOT NULL DEFAULT 0,
            UNIQUE(word, source_lang, target_lang)
        );
        ",
    )?;
    Ok(conn)
}

fn normalize(entry: &NewWord) -> Result<NewWord> {
    let word = entry.word.trim().to_string();
    if word.is_empty() {
        bail!("Nothing to save");
    }
    Ok(NewWord {
        word,
        translation: entry.translation.trim().to_string(),
        source_lang: entry.source_lang.trim().to_string(),
        target_lang: entry.target_lang.trim().to_string(),
        provider: entry.provider.trim().to_string(),
    })
}

fn find(
    conn: &Connection,
    word: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<Option<Word>> {
    let word = word.trim();
    conn.query_row(
        "SELECT id, word, translation, source_lang, target_lang, provider,
                created_at, last_reviewed_at, review_count
         FROM words
         WHERE word = ?1 AND source_lang = ?2 AND target_lang = ?3",
        params![word, source_lang.trim(), target_lang.trim()],
        row_to_word,
    )
    .optional()
    .context("lookup saved word")
}

fn list_at(path: &Path) -> Result<Vec<Word>> {
    let conn = connect(path)?;
    let mut stmt = conn.prepare(
        "SELECT id, word, translation, source_lang, target_lang, provider,
                created_at, last_reviewed_at, review_count
         FROM words
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], row_to_word)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn toggle_at(path: &Path, entry: NewWord) -> Result<bool> {
    let entry = normalize(&entry)?;
    let conn = connect(path)?;
    if let Some(existing) = find(&conn, &entry.word, &entry.source_lang, &entry.target_lang)? {
        conn.execute("DELETE FROM words WHERE id = ?1", params![existing.id])?;
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO words (word, translation, source_lang, target_lang, provider, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            entry.word,
            entry.translation,
            entry.source_lang,
            entry.target_lang,
            entry.provider,
            now_secs(),
        ],
    )?;
    Ok(true)
}

fn delete_at(path: &Path, id: i64) -> Result<()> {
    let conn = connect(path)?;
    let n = conn.execute("DELETE FROM words WHERE id = ?1", params![id])?;
    if n == 0 {
        bail!("Word not found");
    }
    Ok(())
}

fn mark_reviewed_at(path: &Path, id: i64) -> Result<()> {
    let conn = connect(path)?;
    conn.execute(
        "UPDATE words
         SET last_reviewed_at = ?1, review_count = review_count + 1
         WHERE id = ?2",
        params![now_secs(), id],
    )?;
    Ok(())
}

fn row_to_word(row: &rusqlite::Row<'_>) -> rusqlite::Result<Word> {
    Ok(Word {
        id: row.get(0)?,
        word: row.get(1)?,
        translation: row.get(2)?,
        source_lang: row.get(3)?,
        target_lang: row.get(4)?,
        provider: row.get(5)?,
        created_at: row.get(6)?,
        last_reviewed_at: row.get(7)?,
        review_count: row.get(8)?,
    })
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "swtrans-vocab-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("vocab.db")
    }

    #[test]
    fn toggle_saves_and_removes() {
        let path = temp_db();
        let entry = NewWord {
            word: " hello ".into(),
            translation: "你好".into(),
            source_lang: "en".into(),
            target_lang: "zh".into(),
            provider: "Youdao".into(),
        };
        assert!(toggle_at(&path, entry.clone()).unwrap());
        let words = list_at(&path).unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].word, "hello");
        assert_eq!(words[0].translation, "你好");
        assert!(!toggle_at(&path, entry).unwrap());
        assert!(list_at(&path).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn rejects_empty_word() {
        let path = temp_db();
        let err = toggle_at(
            &path,
            NewWord {
                word: "   ".into(),
                translation: "x".into(),
                source_lang: "en".into(),
                target_lang: "zh".into(),
                provider: String::new(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("Nothing to save"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn marks_review() {
        let path = temp_db();
        toggle_at(
            &path,
            NewWord {
                word: "apple".into(),
                translation: "苹果".into(),
                source_lang: "en".into(),
                target_lang: "zh".into(),
                provider: "Dictionary".into(),
            },
        )
        .unwrap();
        let id = list_at(&path).unwrap()[0].id;
        mark_reviewed_at(&path, id).unwrap();
        let word = list_at(&path).unwrap().into_iter().next().unwrap();
        assert_eq!(word.review_count, 1);
        assert!(word.last_reviewed_at.is_some());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
