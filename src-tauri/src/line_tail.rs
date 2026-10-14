//! Keeps only the last lines of a process's diagnostic output.
//!
//! FFmpeg and yt-dlp can print warnings for every frame of a long job. Only the
//! end of that output is useful for an error message (and the job inspector
//! keeps 40 lines), so memory stays bounded however long a job runs.

use std::collections::VecDeque;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::task::JoinHandle;

pub(crate) const DEFAULT_LINES: usize = 200;

pub(crate) struct LineTail {
    lines: VecDeque<String>,
    limit: usize,
}

impl LineTail {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(limit.min(64)),
            limit: limit.max(1),
        }
    }

    pub(crate) fn push(&mut self, line: String) {
        if self.lines.len() == self.limit {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub(crate) fn into_vec(self) -> Vec<String> {
        self.lines.into()
    }
}

/// Reads `stream` to the end on a background task and returns its last
/// non-empty lines.
pub(crate) fn spawn_tail<R>(stream: R) -> JoinHandle<Vec<String>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut tail = LineTail::new(DEFAULT_LINES);
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                tail.push(line);
            }
        }
        tail.into_vec()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_newest_lines_in_order() {
        let mut tail = LineTail::new(3);
        for index in 0..10 {
            tail.push(index.to_string());
        }
        assert_eq!(tail.into_vec(), ["7", "8", "9"]);
    }

    #[tokio::test]
    async fn spawn_tail_skips_blank_lines_and_bounds_output() {
        let text = (0..(DEFAULT_LINES + 50))
            .map(|index| format!("line {index}\n\n"))
            .collect::<String>();
        let lines = spawn_tail(std::io::Cursor::new(text.into_bytes()))
            .await
            .unwrap();
        assert_eq!(lines.len(), DEFAULT_LINES);
        assert_eq!(lines.first().unwrap(), "line 50");
        assert_eq!(
            lines.last().unwrap(),
            &format!("line {}", DEFAULT_LINES + 49)
        );
    }
}
